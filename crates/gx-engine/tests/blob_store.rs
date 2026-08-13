//! The CID-keyed blob store (**M5-05 採(a)**), its ceiling (**M5-20 採(a)**), and the escrow
//! round-trip (**E-M5-6**) — measured by behaviour.
//!
//! req/38 §37:
//!
//! > **M5-05 採(a)**: CID キーの blob store 1 本が `PlannedDelta` と `inverse_delta` の両方を持つ
//! > (M4H6-3「既知 CID は参照のみ」を内包)。GC は DR-9(器のみ・OSS 既定無期限)。
//!
//! > **M5-20 採(a)+(c)**: engine 受取口の decode 前 byte 上限 1 箇所+契約行 1:1 probe(M4H2-8 形)。
//!
//! and req/38 §38:
//!
//! > **M5H1-3 採(a)**=**E-M5-6**: 42 §3.12 の erratum——`inverse_delta` は **`Option`**・status と
//! > payload は checked constructor(`held`/`unavailable`/`restore`)が歩調を保つ。
//!
//! # 「参照のみ」 is measured by what did *not* happen
//!
//! A second `put` of a known CID returning `AlreadyPresent` proves what the function *said*, not
//! what it *did*: an implementation that rewrote the file and then answered `AlreadyPresent` would
//! pass. So the probe below **damages the stored blob first** and then puts the same delta again. A
//! store that rewrote would repair the damage; a store that referenced leaves it. The world is the
//! instrument, which is the shape §30's lesson keeps pointing at — an absence needs a presence to
//! be visible.
//!
//! # The ceiling is measured **on** the bound, both ways
//!
//! req/76 §2.2 recorded a `cargo mutants` survivor whose whole content was 「payload がちょうど
//! 1,048,576 の case を誰も作っていない」: a bound probed only at neighbouring values cannot tell
//! 「at most N」 from 「fewer than N」. So the write side is measured at exactly `MAX_BLOB_BYTES` and
//! at one byte over, and the read side is measured on a file of exactly the ceiling and one byte
//! over it.

mod support;

use gx_canon::cbor;
use gx_canon::cid::IdentityView;
use gx_core::{Cid, SubstrateKind, Timestamp};
use gx_engine::{BlobStore, EscrowRow, InverseStatus, PutOutcome, MAX_BLOB_BYTES};
use gx_substrate::PlannedDelta;
use support::{scratch, tid};

/// A delta with a distinguishable payload.
fn delta(payload: &[u8]) -> PlannedDelta {
    PlannedDelta::new(SubstrateKind::Fs, payload.to_vec()).expect("a small payload is digestible")
}

/// A store in an empty directory.
fn store(name: &str) -> BlobStore {
    BlobStore::open(scratch(name).join("blobs")).expect("a fresh blob store opens")
}

/// The bytes a blob holds: 42 §1.3's projection, which is also the CID's preimage.
fn wire(delta: &PlannedDelta) -> Vec<u8> {
    cbor::encode(&delta.identity_view()).expect("a delta has a canonical projection")
}

// ---------------------------------------------------------------------------
// M5-05 採(a): one store, keyed by the CID the value already carries
// ---------------------------------------------------------------------------

/// A delta is filed under its own CID and comes back through the checked constructor.
///
/// 「CID キー」 is not a convention the store follows — the key is `delta.reference().cid`, which
/// `PlannedDelta::new` minted from the same projection the file holds (M4H1-3 採(a)). So the name of
/// the file is a digest of the contents of the file, and `get` re-mints it on the way back rather
/// than trusting the directory entry.
#[test]
fn a_delta_is_filed_under_its_own_cid_and_rebuilt_through_the_constructor() {
    let s = store("blob_roundtrip");
    let d = delta(b"a change an adapter worked out");

    let (cid, outcome) = s.put(&d).expect("put");
    println!(
        "PUT_CID={} OUTCOME={} STORE_LEN={}",
        gx_canon::cid::to_text(&cid),
        outcome.kind(),
        s.len()
    );
    assert_eq!(cid, d.reference().cid, "the key is the delta's own CID");
    assert_eq!(outcome, PutOutcome::Stored);
    assert!(s.contains(&cid));

    let back = s.get(&cid).expect("get");
    assert_eq!(
        back, d,
        "the delta that comes back is the delta that went in"
    );
    assert_eq!(
        back.reference().cid,
        cid,
        "and it re-mints to the name it was filed under"
    );
}

/// 🔴 **M4H6-3**: a second put of a known CID is a **reference**, and writes nothing.
///
/// The two instruments are in one probe on purpose, because either alone is weak:
///
/// 1. the outcome is `AlreadyPresent` — what the store *says*;
/// 2. a blob damaged between the two puts is **still damaged** afterwards — what the store *did*.
///
/// The damage is not noise: it is the canonical encoding of a *different* delta, so the file stays
/// decodable and only the digest disagrees. That keeps the second half of this probe about writing
/// rather than about parsing, and it sets up
/// [`a_blob_that_does_not_hash_to_its_name_is_refused`] with the same fixture.
#[test]
fn a_second_put_of_a_known_cid_writes_nothing() {
    let s = store("blob_reference_only");
    let mine = delta(b"the one that was stored");
    let (cid, first) = s.put(&mine).expect("first put");
    assert_eq!(first, PutOutcome::Stored);

    let impostor = delta(b"bytes that are not the ones filed here");
    let path = std::fs::read_dir(s.root())
        .expect("the store's directory is readable")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "blob"))
        .expect("one blob is filed");
    std::fs::write(&path, wire(&impostor)).expect("damage the stored blob");

    let (again, second) = s.put(&mine).expect("second put");
    let after = std::fs::read(&path).expect("read the blob back");
    println!(
        "SECOND_PUT={} FILES={} DAMAGE_SURVIVED={}",
        second.kind(),
        s.len(),
        after == wire(&impostor)
    );
    assert_eq!(again, cid);
    assert_eq!(
        second,
        PutOutcome::AlreadyPresent,
        "M4H6-3: 「同一 CID なら参照のみ登録」"
    );
    assert_eq!(
        after,
        wire(&impostor),
        "the second put rewrote the file, so 「参照のみ」 is what the outcome says and not what the \
         store does"
    );
    assert_eq!(s.len(), 1, "one CID, one file");
}

/// The blob whose name and contents disagree is refused (content addressing, from the read side).
///
/// Same fixture as above: a decodable blob under the wrong name. `get` re-mints the CID and refuses,
/// which is the only reason the reference-only rule above is safe — 「the file already there holds
/// the same bytes」 is an assumption, and this is the check that turns it into one that fails loudly.
#[test]
fn a_blob_that_does_not_hash_to_its_name_is_refused() {
    let s = store("blob_digest_check");
    let mine = delta(b"the one that was stored");
    let (cid, _) = s.put(&mine).expect("put");

    let impostor = delta(b"bytes that are not the ones filed here");
    let path = std::fs::read_dir(s.root())
        .expect("readable")
        .flatten()
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "blob"))
        .expect("one blob");
    std::fs::write(&path, wire(&impostor)).expect("swap the contents");

    let refused = s
        .get(&cid)
        .expect_err("a blob that is not its name is refused");
    println!("SWAPPED_BLOB_REFUSAL={} ({refused})", refused.kind());
    assert_eq!(refused.kind(), "Malformed");
    assert!(
        refused.to_string().contains("rebuilds into"),
        "the refusal names what was found rather than only that something was wrong: {refused}"
    );
}

/// 🔴 **M4H6-3 in its own words**: an inverse whose payload equals a forward delta's is one blob.
///
/// req/38 §34: 「residual CID が既存 delta CID と一致するのは**保管の一度性の裏返し**=利点」. The
/// `StubAdapter`'s `invert` returns the delta itself, which is the smallest real instance of that:
/// two *different roles* for one value, and a store that held it twice would be keeping a copy of a
/// thing it already had under the same name.
#[test]
fn an_inverse_with_the_same_payload_is_the_same_blob() {
    let s = store("blob_shared_cid");
    let forward = delta(b"whole-file swap");
    let inverse = delta(b"whole-file swap");

    let (forward_cid, first) = s.put(&forward).expect("put the forward delta");
    let (inverse_cid, second) = s.put(&inverse).expect("put the inverse");
    println!(
        "FORWARD={} INVERSE={} SECOND_OUTCOME={} FILES={}",
        gx_canon::cid::to_text(&forward_cid),
        gx_canon::cid::to_text(&inverse_cid),
        second.kind(),
        s.len()
    );
    assert_eq!(first, PutOutcome::Stored);
    assert_eq!(forward_cid, inverse_cid);
    assert_eq!(second, PutOutcome::AlreadyPresent);
    assert_eq!(s.len(), 1, "one blob for one CID, whichever role asked");
}

/// A CID nobody stored is `NotFound`, which is not `Malformed`.
///
/// req/29 §4's rule at the smallest scale: 「it is not here」 and 「it is here and unreadable」 are
/// different facts, and a store that gave them one face would make a missing body look like a
/// damaged one to the recovery that hand 5 will write against it.
#[test]
fn a_cid_nobody_stored_is_not_found() {
    let s = store("blob_absent");
    let refused = s
        .get(&Cid([9u8; 32]))
        .expect_err("an absent blob is a refusal");
    println!("ABSENT_BLOB_REFUSAL={}", refused.kind());
    assert_eq!(refused.kind(), "NotFound");
}

// ---------------------------------------------------------------------------
// M5-20 採(a): the ceiling, on the bound, from both sides
// ---------------------------------------------------------------------------

/// The write side, **on** the bound: exactly `MAX_BLOB_BYTES` is stored, one byte over is refused.
///
/// The payload that lands the encoding on the number is solved for rather than guessed, the way
/// `gx-adapter-fs/tests/forward_ceiling.rs` does it: measure the framing overhead at a size in the
/// same length class (a byte string of 2^16 or more carries a five-byte header, so the overhead is
/// constant across this region) and then ask for the payload that lands the whole encoding on the
/// ceiling.
#[test]
fn a_blob_of_exactly_the_ceiling_is_stored_and_one_byte_over_is_refused() {
    let s = store("blob_write_ceiling");
    let encoded_len = |payload: usize| wire(&delta(&vec![b'x'; payload])).len();

    let sample = MAX_BLOB_BYTES as usize - 4096;
    let overhead = encoded_len(sample) - sample;
    let exact = MAX_BLOB_BYTES as usize - overhead;
    assert_eq!(
        encoded_len(exact),
        MAX_BLOB_BYTES as usize,
        "the fixture did not land on the bound, so this probe is not about the boundary byte"
    );

    let on_the_bound = delta(&vec![b'x'; exact]);
    let (_, outcome) = s
        .put(&on_the_bound)
        .expect("a blob of exactly the ceiling is at most the ceiling");
    println!(
        "MAX_BLOB_BYTES={MAX_BLOB_BYTES} OVERHEAD={overhead} EXACT_PAYLOAD={exact} \
         ON_THE_BOUND_OUTCOME={}",
        outcome.kind()
    );

    let over = delta(&vec![b'x'; exact + 1]);
    let refused = s
        .put(&over)
        .expect_err("one byte over the ceiling is refused");
    assert_eq!(refused.kind(), "Malformed");
    assert!(
        refused.to_string().contains("ceiling"),
        "the refusal names the ceiling: {refused}"
    );
}

/// 🔴 The read side, **before the decode**: a file over the ceiling is refused by its size.
///
/// This is the whole content of 「decode 前 byte 上限」. The file written here is not a blob at all —
/// it is `MAX_BLOB_BYTES + 1` bytes of a single repeated byte, which is what four corrupted length
/// bytes or a hostile write looks like from the outside. The refusal has to arrive from
/// `Metadata::len`, and it does: the error names the ceiling and the size, and no decoder saw the
/// contents.
#[test]
fn a_file_one_byte_over_the_ceiling_is_refused_before_it_is_decoded() {
    let s = store("blob_read_ceiling");
    let cid = Cid([7u8; 32]);
    let mut name = String::new();
    for byte in cid.0 {
        name.push_str(&format!("{byte:02x}"));
    }
    let path = s.root().join(format!("{name}.blob"));
    std::fs::write(&path, vec![0x41u8; MAX_BLOB_BYTES as usize + 1])
        .expect("write an oversized file");

    let refused = s.get(&cid).expect_err("over the ceiling");
    println!("OVER_CEILING_REFUSAL={} ({refused})", refused.kind());
    assert_eq!(refused.kind(), "Malformed");
    assert!(
        refused.to_string().contains("over the"),
        "the refusal is the ceiling's, not the decoder's: {refused}"
    );

    // The control that makes the line above 「at most」 rather than 「fewer than」: a file of
    // **exactly** the ceiling passes the size check and is refused by the decoder instead. Same
    // bytes, one fewer of them, and a different refusal.
    std::fs::write(&path, vec![0x41u8; MAX_BLOB_BYTES as usize])
        .expect("write a file on the bound");
    let decoded = s.get(&cid).expect_err("garbage is still not a delta");
    println!("ON_BOUND_REFUSAL={} ({decoded})", decoded.kind());
    assert_eq!(decoded.kind(), "Canon");
    assert!(
        !decoded.to_string().contains("over the"),
        "a file of exactly the ceiling was refused by the ceiling, so the bound is read as \
         「fewer than」: {decoded}"
    );
}

// ---------------------------------------------------------------------------
// E-M5-6: the escrow round-trip through the store
// ---------------------------------------------------------------------------

/// 🔴 **E-M5-6 across storage**: `Unavailable` may not name a body, in both directions.
///
/// §38 settled the contradiction in 42 §3.12 (`inverse_delta: PlannedDelta` beside
/// `Unavailable`=「`invert()`がNoneを返した場合」) by making the field an `Option` and the three
/// constructors keep it in step. That held for values built in memory; hand 3 is where they are
/// built from something that was **written down**, and the door is the same one:
/// `BlobStore::escrowed` goes through `EscrowedInverse::restore`.
///
/// Both directions are measured, because they are different lies: a row that says 「no inverse could
/// be constructed」 while holding one, and a row that promises an undo it has no body for.
#[test]
fn a_persisted_escrow_row_cannot_contradict_itself() {
    let s = store("escrow_roundtrip");
    let inverse = delta(b"put the old bytes back");
    let (cid, _) = s.put(&inverse).expect("escrow the body");
    let t = tid(1);

    let held = s
        .escrowed(&EscrowRow {
            transformation: t,
            inverse_cid: Some(cid),
            retained_until: None,
            status: InverseStatus::Available,
        })
        .expect("an available inverse with a body rebuilds");
    assert_eq!(held.transformation(), t);
    assert_eq!(held.inverse_delta(), Some(&inverse));
    assert_eq!(held.status(), &InverseStatus::Available);
    assert_eq!(
        held.retained_until(),
        None,
        "DR-9: the OSS default is 無期限, and the 器 is present"
    );

    let unavailable = s
        .escrowed(&EscrowRow {
            transformation: t,
            inverse_cid: None,
            retained_until: None,
            status: InverseStatus::Unavailable,
        })
        .expect("a row that says no inverse exists, and holds none, rebuilds");
    assert_eq!(unavailable.inverse_delta(), None);
    assert_eq!(unavailable.status(), &InverseStatus::Unavailable);

    let carrying = s
        .escrowed(&EscrowRow {
            transformation: t,
            inverse_cid: Some(cid),
            retained_until: None,
            status: InverseStatus::Unavailable,
        })
        .expect_err("「持っていない」と言いながら保管した store は再構成できない");
    let empty = s
        .escrowed(&EscrowRow {
            transformation: t,
            inverse_cid: None,
            retained_until: None,
            status: InverseStatus::Available,
        })
        .expect_err("an undo promised with no body to run is the other direction");
    println!("ESCROW_REFUSALS=[{}, {}]", carrying.kind(), empty.kind());
    assert_eq!(carrying.kind(), "InconsistentEscrow");
    assert_eq!(empty.kind(), "InconsistentEscrow");
}

/// A `retained_until` a caller supplies survives the round trip — the 器, and no more (DR-9, N-06).
///
/// Nothing in v0.1 enforces the deadline and nothing in the journal carries one (see
/// [`gx_engine::EscrowRow`]); what this probe fixes is that the field is not quietly dropped by the
/// path that rebuilds the row, so the hand that implements the commercial tier finds a seat rather
/// than a hole.
#[test]
fn a_retention_deadline_survives_the_rebuild() {
    let s = store("escrow_deadline");
    let inverse = delta(b"put the old bytes back");
    let (cid, _) = s.put(&inverse).expect("escrow the body");

    let row = EscrowRow {
        transformation: tid(2),
        inverse_cid: Some(cid),
        retained_until: Some(Timestamp(1_754_000_000_000_000_000)),
        status: InverseStatus::Available,
    };
    let rebuilt = s.escrowed(&row).expect("rebuild");
    assert_eq!(rebuilt.retained_until(), row.retained_until);
}
