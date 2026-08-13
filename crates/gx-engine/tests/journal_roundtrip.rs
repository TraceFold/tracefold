//! The journal round-trip req/78 §6.2 手 1 ④ asks for, and the two ways a journal ends badly.
//!
//! > ④ journal の round-trip property(書いた列が読んだ列と一致・torn tail の扱いは gx-log の
//! > `Recovery` と同じ形で報告する)
//!
//! # Why the property is stated on bytes
//!
//! There is no `PartialEq` on [`EngineJournalRecord`]: it holds a written-down fingerprint, and
//! **E-M4-15** took `PartialEq` off `Fingerprint` because 42 §3.5's comparison has three answers
//! and `==` has two (see `store.rs`). Deriving it on the record would hand that third answer back
//! to any caller who reached for `a.fp0 == b.fp0`, undoing a ruling through a type that did not
//! exist when it was made.
//!
//! So 「書いた列が読んだ列と一致」 is measured as **byte equality of the canonical forms**, which for
//! a canonical encoding is the stronger statement: 42 §2.1 makes the form a function of the value,
//! so equal bytes and equal fields are the same fact, and the direction a derived comparison would
//! have asserted is the weaker one.
//!
//! # The two endings
//!
//! A journal file ends either **cleanly** — every record whole — or **torn**, with a partial record
//! at the end because a crash landed inside a write. Those are the only two, and telling them apart
//! is what an audit log owes its reader: 「it has n entries」 is the same number in both cases.
//! [`gx_log::Recovery`] is the shape gx-log settled on for saying which, and this crate reports in
//! it rather than in a second struct of its own.

mod support;

use std::fs;
use std::io::Write;

use gx_canon::cbor;
use gx_core::{Fingerprint, SubstrateKind, Timestamp};
use gx_engine::store::FingerprintRecord;
use gx_engine::{replay, EngineJournal, EngineJournalRecord, Recovery, MAX_RECORD_BYTES};
use proptest::prelude::*;
use support::{cid, every_variant, iid, read_repo, scratch, tid};

fn encoded(records: &[EngineJournalRecord]) -> Vec<Vec<u8>> {
    records
        .iter()
        .map(|r| cbor::encode(r).expect("canonical"))
        .collect()
}

/// One journal, written and read back, answering with the two byte lists.
fn write_then_read(name: &str, records: &[EngineJournalRecord]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let path = scratch(name).join("journal.gxj");
    {
        let mut journal = EngineJournal::open(&path).expect("open a fresh journal");
        for (i, record) in records.iter().enumerate() {
            let seq = journal.append(record.clone()).expect("append");
            assert_eq!(seq, i as u64, "the sequence number is the position");
        }
        assert_eq!(journal.len(), records.len());
    }
    let reopened = EngineJournal::open(&path).expect("reopen");
    assert_eq!(
        reopened.recovery(),
        Recovery {
            records: records.len() as u64,
            torn_tail_bytes: 0
        },
        "a journal closed cleanly reports no torn tail"
    );
    (encoded(records), encoded(reopened.records()))
}

// ---------------------------------------------------------------------------
// The property
// ---------------------------------------------------------------------------

/// A record generator that reaches every variant.
///
/// Hand-written rather than derived: `proptest::arbitrary` would need `Arbitrary` on `Cid`,
/// `Timestamp` and the rest, which is a trait implementation on gx-core's types for a test's
/// convenience — and **M5-15 採(b)** already settled the general question the other way (「素の
/// proptest で…自前 model」, external 238 unchanged). The seeds are the whole of what varies,
/// which is enough: what the property is about is the framing and the encoder, not the values.
fn any_record() -> impl Strategy<Value = EngineJournalRecord> {
    (0u64..64, 0i64..1_000_000).prop_flat_map(|(seed, when)| {
        let at = Timestamp(when);
        prop_oneof![
            Just(EngineJournalRecord::DraftCreated {
                intent_id: iid(seed),
                rng_seed: seed,
                at
            }),
            Just(EngineJournalRecord::Planned {
                transformation: tid(seed),
                intent_id: iid(seed),
                // **E-M5-13**. Derived from the same seed as everything else in this arm, so that
                // the property 「every record round-trips」 covers the two new fields with the same
                // strength it covers the five old ones.
                locator: format!("/tmp/{seed}"),
                delta_cid: cid(seed),
                fp0: support::fp(seed),
                parents: vec![tid(seed + 1)],
                at
            }),
            Just(EngineJournalRecord::VerifyStarted {
                transformation: tid(seed),
                at
            }),
            // Both shapes of the record 43 T-4e added to (M5 hand 2): a verdict the gate reached,
            // carrying a digest, and T-4e's degraded admission, which carries neither. The seed
            // drives both fields together because that is how they come -- a `None` digest with
            // `fail_posture_engaged = false` would be a record no transition writes.
            Just(EngineJournalRecord::Verdict {
                transformation: tid(seed),
                kind: gx_core::VerdictKind::ALL[(seed % 3) as usize],
                verdict_digest: (seed % 2 == 0).then(|| cid(seed)),
                fail_posture_engaged: seed % 2 == 1,
                at
            }),
            Just(EngineJournalRecord::HumanDecision {
                transformation: tid(seed),
                kind: gx_core::VerdictKind::ALL[(seed % 2) as usize],
                reason: format!("ruling {seed}"),
                actor: support::ruler((seed % 251) as u8),
                at
            }),
            Just(EngineJournalRecord::Canonicalized {
                transformation: tid(seed),
                canonical_cid: cid(seed),
                enforced: if seed % 2 == 0 { None } else { Some(false) },
                at
            }),
            Just(EngineJournalRecord::CommittingStarted {
                transformation: tid(seed),
                at
            }),
            Just(EngineJournalRecord::InverseEscrowed {
                transformation: tid(seed),
                inverse_cid: (seed % 3 != 0).then(|| cid(seed)),
                at
            }),
            Just(EngineJournalRecord::ApplyStarted {
                transformation: tid(seed),
                delta_cid: cid(seed),
                at
            }),
            Just(EngineJournalRecord::Committed {
                transformation: tid(seed),
                ledger_seq: seed,
                at
            }),
            Just(EngineJournalRecord::Aborted {
                transformation: tid(seed),
                reason: gx_core::AbortReason::Expired,
                rollback: None,
                at
            }),
            Just(EngineJournalRecord::Superseded {
                transformation: tid(seed),
                by: tid(seed + 1),
                at
            }),
        ]
    })
}

proptest! {
    /// 🔴 The sequence written is the sequence read (**DoD ④**).
    ///
    /// Through a real file, not a buffer: the framing, the fsync and the replay are the thing being
    /// tested, and a round-trip that stayed in memory would test only serde.
    #[test]
    fn a_written_sequence_replays_to_itself(records in prop::collection::vec(any_record(), 0..12)) {
        let path = scratch("roundtrip_property").join("journal.gxj");
        {
            let mut journal = EngineJournal::open(&path).expect("open");
            for record in &records {
                journal.append(record.clone()).expect("append");
            }
        }
        let reopened = EngineJournal::open(&path).expect("reopen");
        prop_assert_eq!(encoded(&records), encoded(reopened.records()));
        prop_assert_eq!(reopened.recovery().torn_tail_bytes, 0);
        prop_assert_eq!(reopened.recovery().records, records.len() as u64);
    }
}

/// The twelve variants, all of them, through a file.
///
/// The property above draws from the same twelve but is not guaranteed to reach each one in any
/// single run. This is the exhaustive companion — the same reason `every_variant()` exists.
#[test]
fn every_variant_survives_a_round_trip() {
    let records = every_variant();
    let (written, read) = write_then_read("roundtrip_all", &records);
    println!("ROUND_TRIPPED_RECORDS={}", read.len());
    assert_eq!(
        written, read,
        "the twelve records did not come back as written"
    );
    assert_eq!(read.len(), 13);
}

/// An empty journal is not a damaged one.
#[test]
fn a_fresh_journal_is_empty_and_undamaged() {
    let path = scratch("roundtrip_empty").join("journal.gxj");
    let journal = EngineJournal::open(&path).expect("open");
    println!("FRESH_RECOVERY={:?}", journal.recovery());
    assert!(journal.is_empty());
    assert_eq!(journal.recovery(), Recovery::default());
    assert!(path.exists(), "opening creates the file");
}

// ---------------------------------------------------------------------------
// The torn tail
// ---------------------------------------------------------------------------

/// 🔴 A journal cut inside its last record replays the prefix and reports the rest.
///
/// Cut at every byte of the last record rather than at one arbitrary point: a crash lands where it
/// lands, and a framing that only survived a cut in the payload — but not one in the length header
/// — would be a framing that works for the case somebody thought of.
#[test]
fn a_cut_journal_replays_its_prefix_and_reports_the_tail() {
    let records = every_variant();
    let path = scratch("roundtrip_torn").join("journal.gxj");
    {
        let mut journal = EngineJournal::open(&path).expect("open");
        for record in &records {
            journal.append(record.clone()).expect("append");
        }
    }
    let whole = fs::read(&path).expect("read the journal back");
    let last_len = 4 + cbor::encode(records.last().expect("twelve records"))
        .expect("canonical")
        .len();

    let mut checked = 0;
    for cut in 1..=last_len {
        let truncated = &whole[..whole.len() - cut];
        let out = replay(truncated);
        assert_eq!(
            out.records().len(),
            records.len() - 1,
            "cutting {cut} bytes should lose exactly the last record"
        );
        assert_eq!(
            out.recovery().torn_tail_bytes,
            (last_len - cut) as u64,
            "the torn tail is what is left of the last record"
        );
        checked += 1;
    }
    println!("TORN_CUTS_CHECKED={checked} LAST_RECORD_BYTES={last_len}");
    assert_eq!(checked, last_len);
}

/// Opening a torn journal removes the tail, and the next open finds a clean file.
///
/// The half that is about the *file* rather than about the bytes: a recovery that reported the tear
/// and left the bytes in place would let the next append extend a contradiction. Reported once,
/// then gone — which is exactly `gx_log::LedgerStore`'s behaviour, and the reason both files can
/// share one [`Recovery`].
#[test]
fn opening_a_torn_journal_truncates_it_once() {
    let records = every_variant();
    let path = scratch("roundtrip_truncate").join("journal.gxj");
    {
        let mut journal = EngineJournal::open(&path).expect("open");
        for record in &records {
            journal.append(record.clone()).expect("append");
        }
    }
    {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append to the journal file");
        file.write_all(&[0u8, 0, 0, 9, 1, 2, 3])
            .expect("write a half record");
    }

    let first = EngineJournal::open(&path).expect("reopen once");
    println!("TORN_OPEN_RECOVERY={:?}", first.recovery());
    assert_eq!(first.recovery().records, 13);
    assert_eq!(first.recovery().torn_tail_bytes, 7);
    drop(first);

    let second = EngineJournal::open(&path).expect("reopen twice");
    println!("CLEAN_OPEN_RECOVERY={:?}", second.recovery());
    assert_eq!(second.recovery().torn_tail_bytes, 0);
    assert_eq!(second.records().len(), 13);
}

/// A journal that was torn can still be appended to, and the new record lands next.
#[test]
fn a_recovered_journal_continues_the_sequence() {
    let path = scratch("roundtrip_continue").join("journal.gxj");
    let mut journal = EngineJournal::open(&path).expect("open");
    journal
        .append(EngineJournalRecord::CommittingStarted {
            transformation: tid(1),
            at: Timestamp(1),
        })
        .expect("append");
    drop(journal);
    {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("open to damage");
        file.write_all(&[0u8, 0, 0, 40]).expect("a lying header");
    }
    let mut recovered = EngineJournal::open(&path).expect("reopen");
    assert_eq!(recovered.recovery().torn_tail_bytes, 4);
    let seq = recovered
        .append(EngineJournalRecord::Aborted {
            transformation: tid(1),
            reason: gx_core::AbortReason::OwnerCancelled,
            rollback: None,
            at: Timestamp(2),
        })
        .expect("append after recovery");
    assert_eq!(seq, 1, "the next record is the second, not the third");
    drop(recovered);
    let final_open = EngineJournal::open(&path).expect("reopen after the append");
    assert_eq!(final_open.records().len(), 2);
    assert_eq!(final_open.recovery().torn_tail_bytes, 0);
}

// ---------------------------------------------------------------------------
// The ceiling (M5-20, write side)
// ---------------------------------------------------------------------------

/// A record over [`MAX_RECORD_BYTES`] is refused before it is written.
///
/// Built through `SubstrateKind::Custom`, which is the one unbounded string a journal record can
/// reach: `Fingerprint::new` bounds the *scope* at `gx_core::MAX_SCOPE_BYTES` and bounds nothing
/// about the substrate's name. That is the shape of the hole M5-20 is about — a receiving mouth
/// whose size is decided by something outside this crate — and the ceiling is where it stops.
#[test]
fn a_record_over_the_ceiling_is_refused() {
    let huge = SubstrateKind::Custom("x".repeat(MAX_RECORD_BYTES as usize + 16));
    let record = EngineJournalRecord::Planned {
        transformation: tid(1),
        intent_id: iid(1),
        locator: "/tmp/x".to_string(),
        delta_cid: cid(1),
        fp0: FingerprintRecord::of(
            &Fingerprint::new(huge, "/tmp/x".to_string(), cid(2)).expect("the scope is short"),
        ),
        parents: Vec::new(),
        at: Timestamp(1),
    };
    let path = scratch("roundtrip_ceiling").join("journal.gxj");
    let mut journal = EngineJournal::open(&path).expect("open");
    let refused = journal.append(record).expect_err("over the ceiling");
    println!("CEILING_REFUSAL={} ({refused})", refused.kind());
    assert_eq!(refused.kind(), "Malformed");
    assert_eq!(
        journal.len(),
        0,
        "a refused record does not enter the journal"
    );
    assert_eq!(
        fs::metadata(&path).expect("the file exists").len(),
        0,
        "a refused record does not reach the file either"
    );
}

/// 🔴 A **well-formed** record over the ceiling is refused on the way back in.
///
/// This probe exists because the first version of the read-side check did not measure the ceiling.
/// `replay.rs`'s unit test frames a lying header over a short buffer, and that is refused whether or
/// not the ceiling is there -- the payload is simply not present, so the length check catches it.
/// The battery found this: mutation (f) removed `length > MAX_RECORD_BYTES` from `replay` and every
/// probe stayed green (`tools/verify_m5h1.sh` §4, first run).
///
/// What discriminates is a record that is **whole, canonical and too big**: with the ceiling it is
/// a torn tail, and without it the journal happily decodes a megabyte the writer would have
/// refused to produce. That asymmetry -- a reader accepting what its own writer rejects -- is the
/// shape M5-20's 「decode 前の byte 上限」 is about, and it is why the ruling puts a ceiling on the
/// *receiving mouth* rather than trusting the producer.
#[test]
fn a_whole_record_over_the_ceiling_is_refused_on_the_way_back_in() {
    let huge = SubstrateKind::Custom("x".repeat(MAX_RECORD_BYTES as usize + 16));
    let record = EngineJournalRecord::Planned {
        transformation: tid(1),
        intent_id: iid(1),
        locator: "/tmp/x".to_string(),
        delta_cid: cid(1),
        fp0: FingerprintRecord::of(
            &Fingerprint::new(huge, "/tmp/x".to_string(), cid(2)).expect("the scope is short"),
        ),
        parents: Vec::new(),
        at: Timestamp(1),
    };
    let payload = cbor::encode(&record).expect("canonical, and larger than the ceiling");
    assert!(payload.len() > MAX_RECORD_BYTES as usize);

    let mut framed = (payload.len() as u32).to_be_bytes().to_vec();
    framed.extend_from_slice(&payload);
    let out = replay(&framed);

    println!(
        "OVERSIZE_RECORD_BYTES={} REPLAYED={} TORN={}",
        payload.len(),
        out.records().len(),
        out.recovery().torn_tail_bytes
    );
    assert_eq!(
        out.records().len(),
        0,
        "a record the writer would refuse must not be accepted by the reader"
    );
    assert_eq!(out.recovery().torn_tail_bytes, framed.len() as u64);
}

// ---------------------------------------------------------------------------
// Write-ahead: the ordering, read off the source
// ---------------------------------------------------------------------------

/// 🔴 43 §7's ordering is three statements in one function, in one order.
///
/// Not observable from outside: a caller cannot tell a record pushed before the fsync from one
/// pushed after until the power fails, and this project does not cut power to a machine to test a
/// vector. So the claim is measured where it is made — the same instrument `gx-log/tests/ac_069.rs`
/// uses for 「durable before visible」, and the same one AC-035 will use in hand 4 for 「apply is
/// called from one place」.
#[test]
fn append_syncs_before_it_makes_a_record_visible() {
    let source = read_repo("crates/gx-engine/src/store.rs");
    let start = source
        .find("pub fn append(&mut self, record: EngineJournalRecord)")
        .expect("append is declared");
    let body = &source[start..];
    let end = body.find("\n    }").expect("the body closes");
    let body = &body[..end];

    let encode = body.find("cbor::encode").expect("the record is encoded");
    let sync = body
        .find("self.write_and_sync")
        .expect("the record is synced");
    let visible = body
        .find("self.records.push")
        .expect("the record becomes visible");

    println!("APPEND_ORDER encode={encode} write_and_sync={sync} push={visible}");
    assert!(
        encode < sync && sync < visible,
        "43 §7 is 「side-effect 実行前に必ず journal 化」 -- the push must follow the barrier"
    );
    assert_eq!(
        body.matches("self.records.push").count(),
        1,
        "one road into the record list"
    );
    assert_eq!(
        source.matches("fn barrier(").count(),
        1,
        "one place that waits for the device (NFR-009's shape, one file over)"
    );
}
