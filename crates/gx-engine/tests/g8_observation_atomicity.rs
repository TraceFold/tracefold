//! 🔴 **`req/859` G8 (E-3) — the crash window in `ObservationStore::put`, opened from the reader's
//! side** (`req/868`, 2026-08-26, seat=Opus, 暫定 — 再審査可).
//!
//! `BlobStore` writes through a temp file and a `rename(2)`; `ObservationStore::put` used to
//! `File::create` at the content address and *then* write into it. A crash between those two steps
//! leaves a **truncated body sitting at its own content address** — a file that lies about its own
//! hash. That is precisely the shape `req/236` H-01 measured and R9 repaired, on the blob store
//! only.
//!
//! **Why this test does not crash the process.** A power cut is not injectable from a `#[test]`,
//! and a test that forked and `SIGKILL`ed a child would be measuring the kernel's page cache rather
//! than our write discipline. But the crash is not the only way through that window: for as long as
//! the window is open, the **final path exists and does not hold the whole body**, and a concurrent
//! reader can see it. A crash makes the partial state permanent; a concurrent read makes the same
//! partial state *observable*. Same window, one observer that a test can actually be.
//!
//! So: one thread stores observations of a fixed size, another lists the directory as fast as it
//! can and measures every published `.obs` file it finds. Every body this test writes is exactly
//! [`BODY`] bytes, so **any** `.obs` file of any other length is a body that was published
//! incomplete. With `File::create` + `write_all` that is reachable (the file is 0 bytes for the
//! whole gap between the two calls). With temp + rename it is unreachable, because the partial
//! write happens under `<cid>.obs.tmp.<pid>` — a name this test does not count and no reader
//! resolves — and `rename(2)` publishes all of it or none of it.
//!
//! **This test is only worth its runtime if it was red first.** The measured red is recorded in
//! `req/868`; if a later lane finds it green against a reintroduced non-atomic `put`, the observer
//! has stopped being fast enough and the test is lying, not passing.

use gx_engine::store::{ObservationStore, PutOutcome};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// One observation body. Well under `MAX_OBSERVATION_BYTES`; the window this test opens is the gap
/// between `create` and the first written byte, which does not depend on the size.
const BODY: usize = 256 * 1024;

/// Enough distinct bodies that a window measured in microseconds is met many times over.
const ROUNDS: usize = 400;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("gx_g8_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

#[test]
fn an_observation_is_never_published_half_written_at_its_own_address() {
    let root = scratch("obs");
    let store = ObservationStore::open(&root).expect("the observation store opens");

    let done = Arc::new(AtomicBool::new(false));
    let partials = Arc::new(AtomicUsize::new(0));
    let looks = Arc::new(AtomicUsize::new(0));

    let observer = {
        let root = root.clone();
        let done = Arc::clone(&done);
        let partials = Arc::clone(&partials);
        let looks = Arc::clone(&looks);
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                if let Ok(entries) = std::fs::read_dir(&root) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) != Some("obs") {
                            continue;
                        }
                        looks.fetch_add(1, Ordering::Relaxed);
                        // The directory entry's own length: no read, no decode, no hash. A
                        // published name whose body is not the whole body is the finding.
                        if let Ok(meta) = entry.metadata() {
                            if meta.len() as usize != BODY {
                                partials.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }
                std::hint::spin_loop();
            }
        })
    };

    let mut stored = 0usize;
    for round in 0..ROUNDS {
        let mut body = vec![0u8; BODY];
        // Distinct bodies, so every round is a fresh CID and a fresh write rather than the
        // content-addressed short circuit.
        body[..8].copy_from_slice(&(round as u64).to_le_bytes());
        let (_cid, outcome) = store
            .put(&body)
            .expect("an observation of the ceiling size stores");
        if outcome == PutOutcome::Stored {
            stored += 1;
        }
    }
    done.store(true, Ordering::Relaxed);
    observer.join().expect("the observer thread finishes");

    let partials = partials.load(Ordering::Relaxed);
    let looks = looks.load(Ordering::Relaxed);
    println!("G8_ROUNDS={ROUNDS} G8_STORED={stored} G8_LOOKS={looks} G8_PARTIALS={partials}");

    // The test must have exercised the writer, or its silence means nothing.
    assert_eq!(
        stored, ROUNDS,
        "every round writes a distinct body, so every round must be a real write and not the \
         already-present short circuit -- if this fails the observer measured nothing"
    );
    assert!(
        looks > 0,
        "the observer never saw a single published .obs file, so its silence about partial ones \
         is vacuous"
    );
    assert_eq!(
        partials, 0,
        "a .obs file was published at its own content address holding fewer (or more) than {BODY} \
         bytes -- req/859 G8: ObservationStore::put must go through write_atomically, so that a \
         partial body lives under <cid>.obs.tmp.<pid> and rename(2) publishes all of it or none"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// 🔴 **`req/871` F4 — the verification half, and this one needs no race at all.**
///
/// Closing the write window stops the product *creating* a truncated body at a content address. It
/// does nothing for a tree that already holds one, because `put` answered `AlreadyPresent` on a
/// bare `path.exists()` — it trusted the **name** where content addressing is a promise about the
/// **bytes**. Such a tree could never heal: every later `put` of the true body would look at the
/// name, agree it was there, and write nothing.
///
/// Unlike the window above, this state can simply be *arranged* — it is the exact residue the old
/// writer left — so this probe is deterministic. `BlobStore::put` has re-read and byte-compared at
/// this point since R9; the assertion below is that both content-addressed stores now do.
#[test]
fn a_truncated_body_already_at_the_address_is_repaired_rather_than_trusted() {
    let root = scratch("repair");
    let store = ObservationStore::open(&root).expect("the observation store opens");

    let body = vec![7u8; BODY];
    let cid = ObservationStore::address(&body);

    // The residue a crash under the old writer left: the right name, a partial body.
    let mut name = String::new();
    for byte in cid.0 {
        use std::fmt::Write;
        let _ = write!(name, "{byte:02x}");
    }
    let path = root.join(format!("{name}.obs"));
    std::fs::write(&path, &body[..1024]).expect("the truncated residue is arranged");
    assert_eq!(
        std::fs::metadata(&path).expect("the residue exists").len(),
        1024,
        "the bed is set: a short body sitting at the full body's content address"
    );

    let (put_cid, outcome) = store.put(&body).expect("the true body stores");
    println!(
        "F4_OUTCOME={outcome:?} F4_LEN={}",
        std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
    );

    assert_eq!(put_cid, cid, "the address is the address of the bytes");
    assert_eq!(
        outcome,
        PutOutcome::Stored,
        "a body at the address that is not the body must be republished, not reported present -- \
         req/871 F4: `path.exists()` trusts the name, and content addressing promises the bytes"
    );
    assert_eq!(
        store
            .get(&cid)
            .expect("the repaired observation reads back"),
        body,
        "after the repair the address must resolve to the whole body"
    );

    let _ = std::fs::remove_dir_all(&root);
}
