//! **ASM-69-1**: what an fs `Fingerprint` covers, and what it deliberately does not.
//!
//! req/38 §28 M4-16 逐語:
//!
//! > 「fs の `scope`=正規化済み絶対 path 1 本(v0.1)・`digest`=**内容のみ**(metadata 除外)。形の理由:
//! > rename 原子 apply は mtime を必ず動かすので、metadata 込み digest は L2(再入安全)と**構造的に矛盾**
//! > する。狭 scope 側の TOCTOU 残存(metadata 面)は doc に明記」
//!
//! The argument is worth restating because it is the reason a *narrow* scope is the right answer
//! here rather than a lazy one. 42 §3.5 recommends widening a scope to 「対象に干渉しうる周辺状態」, and
//! widening costs false aborts; narrowing costs TOCTOU. What settles it for v0.1 is neither
//! preference: hand 5's `apply` gets its atomicity from `rename`, and a rename always writes a new
//! mtime, so a digest that covered metadata would make the second `apply` of one delta observe a
//! different state than the first. That is L2 (re-entry safety), and an adapter cannot hold both.
//!
//! The residue is named in the crate root rather than left implied.

mod support;

use gx_adapter_fs::{normalize, FsAdapter};
use gx_core::MAX_SCOPE_BYTES;
use gx_substrate::SubstrateAdapter;
use support::{Sandbox, SUBJECT};

fn fingerprint_of(adapter: &FsAdapter, locator: &str) -> gx_core::Fingerprint {
    let snapshot = adapter.snapshot(locator).expect("the sandbox holds this");
    adapter.precondition(&snapshot).expect("a scope")
}

/// The scope is the normalised absolute path, and one of them.
#[test]
fn the_scope_is_one_normalised_absolute_path() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let fingerprint = fingerprint_of(&adapter, &locator);
    println!("FS_SCOPE={:?}", fingerprint.scope());
    assert_eq!(fingerprint.scope(), normalize(&locator));
    assert_eq!(fingerprint.substrate(), &gx_core::SubstrateKind::Fs);
    assert!(
        !fingerprint.scope().contains("/./") && !fingerprint.scope().contains("//"),
        "the scope carries a spelling the normalisation would have folded"
    );
}

/// Two spellings of one file are one scope, so a CAS check compares rather than refuses.
///
/// Without this the `Err` arm of `cas_eq` -- 「scope 不一致は adapter 実装エラー」 (E-M4-15) -- would
/// fire on a locator that reached the adapter through `/a/./b` instead of `/a/b`, and an engine
/// would see a bug report where there was a spelling.
#[test]
fn two_spellings_of_one_file_are_one_scope() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let plain = sandbox.locator(SUBJECT);
    let dotted = format!("{}/./{SUBJECT}", sandbox.dir().display());

    let left = fingerprint_of(&adapter, &plain);
    let right = fingerprint_of(&adapter, &dotted);
    assert!(
        left.cas_eq(&right)
            .expect("one scope, so the comparison has a meaning"),
        "two spellings of one file gave two states"
    );
}

/// The digest is the content and nothing else (**ASM-69-1**): rewriting the same bytes does not
/// move it.
///
/// The half that matters for L2. A digest over `(content, mtime)` would move here, and hand 5's
/// `apply` -- which renames, and therefore always writes a new mtime -- would report a different
/// postcondition on its retry than on its first run.
#[test]
fn the_digest_covers_content_and_not_metadata() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);

    let before = fingerprint_of(&adapter, &locator);
    // A write of the same bytes: new mtime, same content.
    sandbox.write(SUBJECT, support::BEFORE);
    let after = fingerprint_of(&adapter, &locator);

    assert!(
        before.cas_eq(&after).expect("one scope"),
        "the fingerprint moved when only metadata did, which is the reading M4-16 rejected"
    );

    sandbox.write(SUBJECT, b"different bytes");
    let moved = fingerprint_of(&adapter, &locator);
    assert!(
        !before.cas_eq(&moved).expect("one scope"),
        "the fingerprint did not move when the content did, so the CAS check of 41 §5-5b would be \
         unfalsifiable (51 §7 契約 3)"
    );
}

/// **M4H1-2**: a scope past the bound arrives as a digest rather than as a long string.
///
/// The path is made real rather than imagined -- a tmpfs takes 255-byte components -- because the
/// claim is about what this adapter does with a file it can actually read, not about what
/// `elide_scope` does with a string.
#[test]
fn a_scope_past_the_bound_is_elided() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();

    let component = "d".repeat(200);
    let mut deep = String::new();
    while sandbox.locator(&deep).len() < MAX_SCOPE_BYTES {
        deep.push_str(&component);
        deep.push('/');
    }
    deep.push_str("leaf");
    sandbox.write(&deep, b"far away");

    let locator = sandbox.locator(&deep);
    assert!(
        locator.len() > MAX_SCOPE_BYTES,
        "the path is past the bound"
    );

    let fingerprint = fingerprint_of(&adapter, &locator);
    let scope = fingerprint.scope();
    println!("FS_LONG_SCOPE_BYTES={} SCOPE={scope}", locator.len());
    assert!(
        scope.len() <= MAX_SCOPE_BYTES,
        "an over-long scope reached a `Fingerprint`, which M4H1-2 bounds at {MAX_SCOPE_BYTES} bytes"
    );
    assert!(
        scope.starts_with("<elided:") && scope.contains("gx1:"),
        "the elision does not name what it replaced: {scope:?}"
    );
}

/// The snapshot reports the locator it was asked about, normalised (H-2).
#[test]
fn a_snapshot_reports_the_normalised_locator() {
    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let dotted = format!("{}//{SUBJECT}", sandbox.dir().display());

    let snapshot = adapter.snapshot(&dotted).expect("the sandbox holds this");
    assert_eq!(snapshot.locator(), sandbox.locator(SUBJECT));
    assert_eq!(snapshot.substrate(), &gx_core::SubstrateKind::Fs);
}

/// A locator this adapter will not accept is refused where it is used, not where it is spelled.
#[test]
fn a_relative_locator_is_refused_by_the_methods_that_need_a_position() {
    let adapter = FsAdapter::new();
    let refusal = adapter
        .snapshot("relative/path")
        .expect_err("v0.1 names positions from the root (ASM-69-3)");
    assert_eq!(refusal.kind(), "Unreadable");

    let sandbox = Sandbox::new();
    let pre = adapter
        .snapshot(&sandbox.locator(SUBJECT))
        .expect("a snapshot");
    let refusal = adapter
        .plan(&support::intent_for("relative/path", b"x"), &pre)
        .expect_err("planning against a position that is not one");
    assert_eq!(refusal.kind(), "NotPlannable");
}
