//! The fs delta grammar: canonical DAG-CBOR, a free monoid, and exactly one operation in v0.1.
//!
//! # Three rulings, one payload
//!
//! * **M4-13 採(a)** (req/38 §28): 「v0.1 の fs delta は **単一 file 全置換**・原子性は rename…
//!   **v0.1 の `apply` は len==1 の列のみ受理**・len>1 は Err(未対応の明示・黙って非原子実行しない=
//!   fail-closed)」.
//! * **M4-07 採(c)**: the composite is a **free monoid** in the payload -- 「fs payload を「単一 file
//!   操作の列」とし、列の連結が合成の witness」 -- so the shape is a sequence even while only one
//!   length is accepted. A grammar that admitted a single operation and nothing else would have no
//!   room for the composition the ruling named.
//! * **N-14** (req/69 §1): 「fs delta の payload は canonical DAG-CBOR(42 §3.4)なので既存
//!   `fuzz_dagcbor_decode` の射程内——**adapter 固有の parser を作るなら話が変わる**」. So this adapter
//!   writes no parser: it derives `Serialize`/`Deserialize` and hands the bytes to gx-canon, which is
//!   also what keeps 41 §6's 「canonical encode は gx-canon を通る」 true of an adapter's own grammar.
//!
//! # What 「連結が witness」 means for a CBOR array
//!
//! The mock of hand 3 used a framed byte format, where concatenating two payloads concatenated two
//! sequences. Canonical DAG-CBOR cannot do that -- an array carries its length in the head -- so the
//! monoid operation is on the **sequences**, and the payload is what one sequence encodes to. That is
//! the honest reading of M4-07(c) under N-14, and [`concatenation_is_the_composition`] measures the
//! associativity that makes it a monoid at all.

mod support;

use gx_adapter_fs::{FsDelta, FsOp, MAX_OPS};
use gx_canon::cbor;

fn op(locator: &str, content: &[u8]) -> FsOp {
    FsOp::write(locator.to_string(), content.to_vec())
}

/// The payload is canonical DAG-CBOR, judged by the encoder rather than by a parse (ASM-01-2).
#[test]
fn the_payload_is_canonical_dag_cbor() {
    let payload = FsDelta::one(op("/tmp/x", b"after"))
        .encode()
        .expect("a one-operation sequence has a canonical form");
    println!("FS_DELTA_PAYLOAD_BYTES={}", payload.len());
    assert!(
        cbor::is_canonical(&payload),
        "the payload is not what gx-canon's encoder would have written, so two byte strings could \
         name one delta (42 §2.1)"
    );
}

/// It round-trips through gx-canon, and this adapter wrote no parser to do it (**N-14**).
#[test]
fn the_grammar_round_trips_through_gx_canon() {
    let original = FsDelta::one(op("/tmp/x", b"after"));
    let payload = original.encode().expect("an encoding");
    let back = FsDelta::decode(&payload).expect("the adapter reads its own grammar");
    assert_eq!(back, original);
    assert_eq!(back.ops()[0].locator(), "/tmp/x");
    assert_eq!(back.ops()[0].content(), Some(b"after".as_slice()));
}

/// A removal is a distinct operation and survives the round trip.
///
/// AC-049 (hand 5) asks for 「作成/変更/削除の 3 種」, so the grammar has to be able to say all three
/// before hand 5 can plan them. Hand 4 plans only the replacement -- an intent carries a goal, and a
/// goal of 「nothing」 has no spelling in 42 §3.3 -- which is recorded here rather than left as a
/// silence.
#[test]
fn a_removal_has_a_spelling_of_its_own() {
    let removal = FsDelta::one(FsOp::remove("/tmp/x".to_string()));
    let payload = removal.encode().expect("an encoding");
    assert!(cbor::is_canonical(&payload));
    let back = FsDelta::decode(&payload).expect("a removal reads back");
    assert_eq!(back.ops()[0].content(), None);
    assert_ne!(
        payload,
        FsDelta::one(op("/tmp/x", b""))
            .encode()
            .expect("an encoding"),
        "「remove the file」 and 「make the file empty」 are two changes and must not be one payload"
    );
}

/// **M4-13(a)**: a sequence longer than one is refused, loudly, and not run half-way.
#[test]
fn a_longer_sequence_is_refused_rather_than_run() {
    let two = FsDelta::of(vec![op("/tmp/x", b"one"), op("/tmp/y", b"two")]);
    let payload = two.encode().expect("the grammar can say it");
    let refusal = FsDelta::decode(&payload).expect_err("v0.1 accepts one operation");

    println!("FS_DELTA_LEN2_REFUSAL={}", refusal.kind());
    assert_eq!(
        refusal.kind(),
        "Unimplemented",
        "a two-operation sequence is not malformed, it is unsupported: 「黙って非原子実行しない=\
         fail-closed」 (M4-13(a)), and 45 §3 names a multi-file `apply` as TH-3's residual condition"
    );
    assert_eq!(MAX_OPS, 1, "the bound v0.1 declares, in one place");
}

/// An empty sequence is refused too, and as a different thing.
///
/// The unit of the free monoid is a legal *value* and not a legal *v0.1 payload*: it describes no
/// file operation, so it cannot be applied and cannot be inverted. That is a payload this adapter
/// would never have written, which is what [`gx_substrate::Error::PayloadUnreadable`] is for --
/// unlike the two-operation case, which the adapter could write once hand 5's successor supports it.
#[test]
fn the_empty_sequence_is_not_a_v0_1_payload() {
    let payload = FsDelta::of(Vec::new()).encode().expect("the unit encodes");
    let refusal = FsDelta::decode(&payload).expect_err("v0.1 needs exactly one operation");
    assert_eq!(refusal.kind(), "PayloadUnreadable");
}

/// The monoid: concatenating sequences is associative, and the empty sequence is its unit.
///
/// **M4-07 採(c)** is the ruling this measures -- 「形は **free monoid**…結合律だけ」 -- and it is
/// measured on the sequences rather than on the bytes, for the reason the module documentation
/// gives. Nothing here claims the general law the crate root explicitly refuses (「一般法則(合成
/// arrow の delta=部分の合成)は主張しない」): this is associativity of concatenation, which is the
/// whole of what a free monoid promises.
#[test]
fn concatenation_is_the_composition() {
    let a = vec![op("/tmp/a", b"1")];
    let b = vec![op("/tmp/b", b"2")];
    let c = vec![op("/tmp/c", b"3")];

    let left = FsDelta::of([a.clone(), b.clone()].concat());
    let left = FsDelta::of([left.ops().to_vec(), c.clone()].concat());
    let right = FsDelta::of([b.clone(), c.clone()].concat());
    let right = FsDelta::of([a.clone(), right.ops().to_vec()].concat());
    assert_eq!(left, right, "(a·b)·c and a·(b·c) are the same sequence");

    let unit = FsDelta::of(Vec::new());
    assert_eq!(
        FsDelta::of([a.clone(), unit.ops().to_vec()].concat()),
        FsDelta::of(a.clone()),
        "the empty sequence is the unit"
    );
}

/// Bytes that are not this grammar are refused as a payload, not as a crash.
#[test]
fn foreign_bytes_are_refused() {
    for bytes in [b"".as_slice(), b"not cbor at all", &[0xffu8, 0xff]] {
        let refusal = FsDelta::decode(bytes).expect_err("these are not an fs delta");
        assert_eq!(refusal.kind(), "PayloadUnreadable", "for {bytes:?}");
    }
}

/// A relative locator is refused as **not a position**, and not as a failed application
/// (**M4H5-5 採(b)**).
///
/// req/38 §33 逐語: 「**`NotAPosition`** variant を追加(相対 locator の拒否は「適用失敗」でなく「引数が
/// 位置でない」——ApplyFailed の流用は事実の誤記=Unimplemented と同じ三悪論法)」. Hand 5 spelled this
/// [`gx_substrate::Error::ApplyFailed`] and raised it against itself (req/74 §2 M4H5-5); the word
/// exists now, and this is the probe that keeps the fact and its name together. A relative locator is
/// a legal **value** of the grammar -- L7 is defined over every string -- and an illegal thing to act
/// on (**ASM-69-3**), which is why the refusal lives at [`FsOp::position`] and not in `decode`.
#[test]
fn a_relative_position_is_refused_as_not_a_position() {
    let refusal = op("relative/x", b"after")
        .position()
        .expect_err("v0.1 names positions from the root");
    println!(
        "FS_OP_RELATIVE_REFUSAL={} MESSAGE={refusal}",
        refusal.kind()
    );
    assert_eq!(
        refusal.kind(),
        "NotAPosition",
        "「the argument is not a position」 and 「the delta could not be applied」 are different \
         facts, and 43 T-11 turns the second into `AbortReason::ApplyFailed`"
    );
    assert!(
        refusal.to_string().contains("relative/x"),
        "the refusal does not name the spelling that was refused: {refusal}"
    );
    assert_eq!(
        op("/absolute/x", b"after")
            .position()
            .expect("an absolute locator is a position"),
        "/absolute/x",
        "the control: the same call on a position answers with the normalised spelling"
    );
}

/// The delta an adapter plans carries exactly this payload, so the grammar is not a second story.
#[test]
fn the_planned_delta_carries_the_grammar() {
    use gx_adapter_fs::FsAdapter;
    use support::{Sandbox, GOAL, SUBJECT};

    let sandbox = Sandbox::new();
    let adapter = FsAdapter::new();
    let locator = sandbox.locator(SUBJECT);
    let delta = support::planned(&adapter, &locator, GOAL);

    let decoded = FsDelta::decode(delta.payload()).expect("the adapter reads what it wrote");
    assert_eq!(decoded.ops().len(), MAX_OPS);
    assert_eq!(decoded.ops()[0].locator(), locator);
    assert_eq!(decoded.ops()[0].content(), Some(GOAL));
}
