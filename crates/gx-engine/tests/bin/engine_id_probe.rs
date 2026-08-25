// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! AC-030's second process: `submit` and `plan` in a program that shares nothing with the test.
//!
//! 34 AC-030, verbatim (sem: SEM-gx-engine-616): "call `gx_engine::submit(I)` as a library
//! function twice within the same process, and **once in a completely independent separate
//! process**. Then: all three calls return the same `IntentId`" -- and the same for
//! `plan` and `TransformationId`. 34's own AC-011 row is the precedent for what "completely independent"
//! means and how it is arranged ("separate binary, no shared cache, separate working directory"), so this
//! file is `gx-canon/tests/bin/cid_probe.rs`'s shape one crate over.
//!
//! # Why a whole binary rather than a thread
//!
//! What AC-030 is protecting is the claim that an id is a function of the value. A second thread
//! shares the address space, the allocator, the hash seeds and any lazily-initialised state, so a
//! second thread agreeing is nearly no evidence at all. A second *process* shares none of them, and
//! this one is started with `env_clear()` in a fresh working directory, which removes the two
//! remaining channels a wrong implementation could have used to agree by accident.
//!
//! # Protocol
//!
//! stdin: one line of JSON, `{"locator": "...", "goal": "..."}`. stdout, on success:
//!
//! ```text
//! INTENT_ID=gx1:...
//! TRANSFORMATION_ID=gx1:...
//! ```
//!
//! Anything else is a failure and the exit code says so. The fixture is built **here** from the two
//! strings rather than deserialised from the parent, so the two processes agree on a *value* and not
//! on a serialisation of one -- deserialising would let a round-trip bug make the ids agree for the
//! wrong reason.

#![forbid(unsafe_code)]

use std::io::Read;

use gx_core::{
    Actor, ChangeContext, Cid, Fingerprint, GoalBytes, Intent, ObjectId, ObjectSnapshot, ReprKind,
    SubstrateKind, Timestamp,
};
use gx_engine::{Engine, InjectedEvidence};
use gx_substrate::{PlannedDelta, SubstrateAdapter};

/// The stub adapter, spelled again rather than shared with `tests/support/mod.rs`.
///
/// A binary and an integration test are separate compilation units, and the module under `tests/`
/// cannot be reached from here without making it a published part of the crate. Duplicating ~30
/// lines is the smaller cost -- and it is arguably the honest one: the second process is supposed to
/// share as little as possible with the first, and sharing the adapter would be sharing the thing
/// whose output the id is computed from.
struct StubAdapter;

impl SubstrateAdapter for StubAdapter {
    fn kind(&self) -> SubstrateKind {
        SubstrateKind::Fs
    }

    fn snapshot(&self, locator: &str) -> gx_substrate::Result<ObjectSnapshot> {
        Ok(ObjectSnapshot::new(
            ObjectId(digest_of(locator.as_bytes())),
            SubstrateKind::Fs,
            locator.to_string(),
            digest_of(locator.as_bytes()),
            ReprKind::Bytes,
        ))
    }

    fn plan(&self, intent: &Intent, _pre: &ObjectSnapshot) -> gx_substrate::Result<PlannedDelta> {
        PlannedDelta::new(SubstrateKind::Fs, intent.goal().0.clone())
    }

    fn precondition(&self, snap: &ObjectSnapshot) -> gx_substrate::Result<Fingerprint> {
        Ok(Fingerprint::new(
            SubstrateKind::Fs,
            snap.locator().to_string(),
            *snap.digest(),
        )?)
    }

    fn apply(&self, _delta: &PlannedDelta) -> gx_substrate::Result<gx_substrate::AppliedDelta> {
        // Never called: nothing in this binary reaches T-11, and FR-035 keeps the engine out of the
        // substrate anyway. Refusing is more honest than a plausible answer.
        Err(gx_substrate::Error::Unimplemented {
            method: "apply".to_string(),
            detail: "AC-030's probe never commits".to_string(),
        })
    }

    fn invert(
        &self,
        delta: &PlannedDelta,
        _pre: &ObjectSnapshot,
    ) -> gx_substrate::Result<gx_substrate::InvertOutcome> {
        Ok(gx_substrate::InvertOutcome::inverted(
            delta.clone(),
            Vec::new(),
        ))
    }

    fn commutation(
        &self,
        _a: &PlannedDelta,
        _b: &PlannedDelta,
    ) -> gx_substrate::Result<gx_core::Commutation> {
        Ok(gx_core::Commutation::Commutes)
    }
}

/// A distinguishable digest of some bytes. Not a hash of anything meaningful, and deterministic
/// across processes, which is the only property AC-030 needs of it.
fn digest_of(bytes: &[u8]) -> Cid {
    let mut raw = [0u8; 32];
    for (i, b) in bytes.iter().enumerate() {
        raw[i % 32] ^= *b;
    }
    Cid(raw)
}

fn main() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("engine_id_probe: cannot read stdin");
        std::process::exit(2);
    }

    let Some(locator) = field(&input, "locator") else {
        eprintln!("engine_id_probe: no locator in {input:?}");
        std::process::exit(2);
    };
    let Some(goal) = field(&input, "goal") else {
        eprintln!("engine_id_probe: no goal in {input:?}");
        std::process::exit(2);
    };

    let intent = Intent::new(
        SubstrateKind::Fs,
        locator,
        GoalBytes(goal.into_bytes()),
        ChangeContext::Policy,
        Actor::Agent {
            key: "key-agent-1".to_string(),
            model: "claude-fable-5".to_string(),
        },
    );

    let dir = std::env::temp_dir().join(format!("gx-engine-probe-{}", std::process::id()));
    if std::fs::create_dir_all(&dir).is_err() {
        eprintln!("engine_id_probe: cannot make a working directory");
        std::process::exit(2);
    }

    let mut engine = match Engine::open(
        dir.join("journal.bin"),
        gx_gate::Gate::unconfigured(),
        InjectedEvidence::none(),
    ) {
        Ok(engine) => engine,
        Err(e) => {
            eprintln!("engine_id_probe: cannot open the journal: {e}");
            std::process::exit(3);
        }
    };
    engine.register_adapter(std::sync::Arc::new(StubAdapter), "stub-1");

    let at = Timestamp(1_754_000_000_000_000_000);
    let intent_id = match engine.submit(&intent, 42, at) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("engine_id_probe: submit refused: {e}");
            std::process::exit(3);
        }
    };
    let transformation_id = match engine.plan(&intent, at) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("engine_id_probe: plan refused: {e}");
            std::process::exit(3);
        }
    };

    println!("INTENT_ID={}", gx_canon::cid::to_text(&intent_id.0));
    println!(
        "TRANSFORMATION_ID={}",
        gx_canon::cid::to_text(&transformation_id.0)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The value of `"<name>": "<value>"` in the one-line JSON the parent writes.
///
/// A three-line reader rather than a `serde_json` dependency: the input is this binary's own test
/// fixture, the shape is fixed by the caller twenty lines away, and **external 238 must not move**
/// (M4H1-6's standing rule). `gx-canon/tests/bin/cid_probe.rs` could use `serde_json` because
/// gx-canon already ships it; gx-engine does not.
fn field(input: &str, name: &str) -> Option<String> {
    let needle = format!("\"{name}\"");
    let rest = input.split_once(&needle)?.1;
    let rest = rest.split_once(':')?.1;
    let rest = rest.trim_start().strip_prefix('"')?;
    let (value, _) = rest.split_once('"')?;
    Some(value.to_string())
}
