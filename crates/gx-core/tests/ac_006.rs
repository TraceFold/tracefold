//! AC-006 (FR-006) — every `Actor` variant carries the same public-key reference type.
//!
//! AC-006 逐語: 「Given: `Actor::Human{key}`, `Actor::Agent{key, model:"claude-x"}`,
//! `Actor::Process{key}`の3variant。When: `key`フィールドの型を比較するジェネリック関数へ
//! 全variantを渡す。Then: 3variantとも同一の公開鍵参照型（例: `PubKeyRef`）でコンパイルが
//! 通過する。」
//!
//! The AC writes the type name as an example (「例: `PubKeyRef`」); 42 §3.2 fixes it as `KeyId`,
//! sharing a namespace with the DSSE `keyid` field, so this file uses that name. What the AC
//! actually pins is the *sameness*, and `same_type` below is where that is enforced: a single
//! type parameter over three arguments does not compile unless all three agree.

use gx_core::{Actor, KeyId};

/// One type parameter, three arguments. If `Human.key` and `Process.key` ever drifted apart,
/// this call would fail to infer and the test would stop building -- which is the compile-time
/// check the AC asks for, rather than an assertion evaluated at run time.
fn same_type<T>(_a: &T, _b: &T, _c: &T) {}

#[test]
fn ac_006_three_variants_one_key_type() {
    let human = Actor::Human {
        key: "k-human".to_string(),
    };
    let agent = Actor::Agent {
        key: "k-agent".to_string(),
        model: "claude-x".to_string(),
    };
    let process = Actor::Process {
        key: "k-process".to_string(),
    };

    let (hk, ak, pk) = match (&human, &agent, &process) {
        (
            Actor::Human { key: hk },
            Actor::Agent { key: ak, model: _ },
            Actor::Process { key: pk },
        ) => (hk, ak, pk),
        _ => unreachable!("constructed immediately above"),
    };
    same_type(hk, ak, pk);

    // Same again through one accessor: a signature that does not mention the variant.
    same_type(human.key(), agent.key(), process.key());
    assert_eq!(human.key(), "k-human");
    assert_eq!(agent.key(), "k-agent");
    assert_eq!(process.key(), "k-process");
}

#[test]
fn ac_006_key_type_is_the_declared_alias() {
    // Annotated with `KeyId` and nothing else. If `Actor::key` returned some other type this
    // binding would not type-check (42 §3.2: `KeyId = String`, the DSSE keyid namespace).
    let a = Actor::Agent {
        key: "k".to_string(),
        model: "claude-x".to_string(),
    };
    let k: &KeyId = a.key();
    assert_eq!(k, "k");

    // The `model` field belongs to `Agent` alone -- the AC lists it only there.
    match &a {
        Actor::Agent { model, .. } => assert_eq!(model, "claude-x"),
        _ => unreachable!(),
    }
}
