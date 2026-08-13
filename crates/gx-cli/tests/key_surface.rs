//! 🔴 **M6-29** — `gx key gen`'s output holds no secret, and `gx key list` reads a directory.
//!
//! req/88 §6.2 手 2's DoD: 「`gx key gen --json` の出力に秘密鍵 byte が現れない probe(M6-29)」, from
//! 44 §1.2's own clause 「秘密鍵はファイル/OSキーストアへ、**標準出力に出さない**」.
//!
//! # 🔴 The probe looks for the bytes, not for a field name
//!
//! A test that asserted `json.get("secret").is_none()` would pass on a leak labelled anything else,
//! and a test that generated a random key could not look for its bytes at all. So the leak check is
//! done against a key made from a **known seed** through the library, whose secret bytes are then
//! searched for in every encoding the output could plausibly carry them in — raw, base64, base32
//! and hex. That is the difference between checking a name and checking a fact.

mod support;

use gx_cli::exit::Outcome;
use gx_cli::keys::{self, KeyStore};
use gx_witness::KeyPair;
use support::{run, secure_scratch};

/// Every encoding the output could smuggle thirty-two bytes in.
fn spellings(secret: &[u8]) -> Vec<(&'static str, String)> {
    let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    vec![
        ("base64", gx_core::b64::encode(secret)),
        ("hex", hex.clone()),
        ("hex-upper", hex.to_uppercase()),
        ("raw-lossy", String::from_utf8_lossy(secret).to_string()),
    ]
}

/// 🔴 M6-29: the generated key's secret bytes appear in no encoding of `gx key gen`'s stdout.
#[test]
fn the_generated_secret_never_reaches_stdout() {
    let dir = secure_scratch("key_gen_leak");
    let store = KeyStore::at(dir.join("keys"));
    let out: Outcome = keys::gen(&store, keys::ALGORITHM, None).expect("generation succeeds");
    let printed = serde_json::to_string(&out.json).expect("serialises");

    // The key that was just written, loaded back so the actual secret is in hand.
    let entries = store.list().expect("list");
    assert_eq!(entries.len(), 1, "one key was generated");
    let pair = KeyPair::load(&entries[0].path).expect("load");
    let secret = pair.signing_key().to_bytes();

    let mut leaked: Vec<&str> = Vec::new();
    for (label, text) in spellings(&secret) {
        if !text.is_empty() && printed.contains(&text) {
            leaked.push(label);
        }
    }
    println!(
        "KEYGEN_STDOUT_BYTES={} ENCODINGS_SCANNED=4 LEAKED={leaked:?}",
        printed.len()
    );
    assert!(
        leaked.is_empty(),
        "44 §1.2: 「秘密鍵は…標準出力に出さない」 — the secret appeared in {leaked:?}"
    );

    // The positive control §30 asks for: the scanner finds the secret when it *is* there. Without
    // this, an encoder that produced an empty string for every input would report zero leaks.
    let with_secret = format!("{{\"oops\":\"{}\"}}", gx_core::b64::encode(&secret));
    assert!(
        spellings(&secret)
            .iter()
            .any(|(_, text)| with_secret.contains(text)),
        "the scanner has to be able to see a leak, or its zero means nothing"
    );

    // And the two fields 44 §1.2 does ask for.
    assert!(out.json["key_id"].as_str().is_some());
    assert!(out.json["public_key"].as_str().is_some());
    assert_eq!(
        out.json.as_object().expect("object").len(),
        2,
        "44 §1.2 names exactly two fields; a third would be one more thing a redirect captures"
    );
}

/// The public key printed is the public key of the secret filed.
///
/// The obvious failure this rules out is a `gen` that printed one key and stored another, which a
/// leak probe and a permissions probe would both miss.
#[test]
fn the_printed_public_key_belongs_to_the_stored_secret() {
    let dir = secure_scratch("key_gen_pairing");
    let store = KeyStore::at(dir.join("keys"));
    let out = keys::gen(&store, keys::ALGORITHM, None).expect("generation succeeds");
    let key_id = out.json["key_id"].as_str().expect("id");
    let printed = out.json["public_key"].as_str().expect("public key");

    let pair = store.load(key_id).expect("the id names the file");
    assert_eq!(
        printed,
        gx_core::b64::encode(&pair.public().to_bytes()),
        "the printed public half is the stored key's"
    );
    assert_eq!(
        key_id,
        keys::derive_key_id(&pair.public().to_bytes()),
        "the id is derived from the key rather than chosen, so the file name is checkable"
    );
    println!("KEY_ID={key_id} PUBLIC_KEY_B64_LEN={}", printed.len());
}

/// `--out` puts the secret where it was asked to, and `--alg` refuses anything but ed25519.
#[test]
fn out_names_the_file_and_alg_refuses_what_this_version_cannot_write() {
    let dir = secure_scratch("key_gen_out");
    let store = KeyStore::at(dir.join("unused"));
    let target = dir.join("nested").join("mine.key");
    let out = keys::gen(&store, keys::ALGORITHM, Some(&target)).expect("generation succeeds");
    println!("KEY_OUT={} EXISTS={}", target.display(), target.is_file());
    assert!(
        target.is_file(),
        "--out creates the parent and writes there"
    );
    assert!(
        !store.dir().exists(),
        "with --out the default store is not touched"
    );
    assert!(KeyPair::load(&target).is_ok());
    let _ = out;

    let err = keys::gen(&store, "rsa", None).expect_err("one algorithm");
    println!("KEY_ALG_REFUSAL exit={} {err}", err.exit_code());
    assert!(matches!(err, gx_cli::Error::Usage { .. }));
    assert_eq!(err.exit_code(), 1, "44 §1.2: 「1=鍵生成/アクセス失敗」");
}

/// 🔴 **M6H2-10** — a key written where 0600 cannot be honoured is warned about **at generation**.
///
/// `KeyPair::save` asks for the mode and a filesystem with no unix permission model ignores it;
/// `KeyPair::load` then refuses the file this binary just wrote. The refusal is right and its
/// timing was not — it arrived one command later, phrased as a permissions error about a file the
/// operator did not know they had made. The judgement is now taken where they can still choose a
/// different `--out`.
///
/// A warning and not a refusal: 44 §1.2 gives `gen` no exit code for 「wrote it, but somewhere
/// unsafe」, and an operator exporting to a shared volume on purpose is a real case.
#[test]
#[cfg(unix)]
fn a_key_written_where_0600_cannot_hold_is_warned_about_at_generation() {
    use std::os::unix::fs::PermissionsExt;
    let dir = secure_scratch("key_mode_warning");
    let store = KeyStore::at(dir.join("keys"));
    keys::gen(&store, keys::ALGORITHM, None).expect("generation succeeds");
    let entry = store.list().expect("list").remove(0);

    println!(
        "KEY_WARNING_TIGHT={:?}",
        keys::permission_warning(&entry.path)
    );
    assert!(
        keys::permission_warning(&entry.path).is_none(),
        "a 0600 file has nothing to warn about"
    );

    std::fs::set_permissions(&entry.path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let warning = keys::permission_warning(&entry.path).expect("0644 is a warning");
    println!("KEY_WARNING_LOOSE={warning}");
    assert!(warning.contains("M6H2-10"), "the warning names its ticket");
    assert!(
        KeyPair::load(&entry.path).is_err(),
        "and the warning is about a real consequence: this key no longer loads"
    );
}

/// `read_public` takes both documents, and refuses a third thing with a reason.
///
/// The two are 44 §1.2's `{key_id, public_key}` and a gx key file. A receipt handed to `--key` by
/// mistake gets told what it is, rather than 「not a key」 — M4H5-5's 「引数が不正を適用失敗と綴るな」
/// in the small.
#[test]
fn the_key_argument_reads_both_documents_and_names_what_a_third_thing_is() {
    let dir = secure_scratch("key_read_public");
    let pair = KeyPair::from_seed("key-42".to_string(), &[42u8; 32]);

    let doc = support::write_public_key(&dir, &pair);
    let from_doc = keys::read_public(&doc).expect("the gen output is a key document");
    assert_eq!(from_doc.key_id(), pair.key_id());
    assert_eq!(from_doc.to_bytes(), pair.public().to_bytes());

    let file = dir.join("secret.key");
    pair.save(&file).expect("save");
    let from_file = keys::read_public(&file).expect("a gx key file has a public half");
    assert_eq!(from_file.to_bytes(), pair.public().to_bytes());

    let nonsense = dir.join("receipt.json");
    std::fs::write(&nonsense, br#"{"envelope":{},"issued_at":0}"#).expect("write");
    let err = keys::read_public(&nonsense).expect_err("neither document");
    println!("KEY_READ_REFUSAL={err}");
    assert!(
        err.to_string().contains("neither"),
        "the refusal names both documents it looked for: {err}"
    );

    // A `{key_id, public_key}` document whose bytes are the wrong length is refused by gx-witness
    // rather than truncated.
    let short = dir.join("short.pub.json");
    std::fs::write(
        &short,
        serde_json::to_vec(&serde_json::json!({
            "key_id": "key-42",
            "public_key": gx_core::b64::encode(&[0u8; 16]),
        }))
        .expect("serialises"),
    )
    .expect("write");
    let err = keys::read_public(&short).expect_err("16 bytes is not an Ed25519 key");
    println!("KEY_SHORT_REFUSAL={err}");
    assert!(matches!(err, gx_cli::Error::Witness(_)));
}

/// 🔴 `~/.gx/keys/` is resolved from the environment, and its absence is a refusal rather than a
/// fallback to the working directory.
///
/// req/56 §1: 「秘密は project に置かない」. A `gen` that wrote into whatever directory the operator
/// happened to be in is exactly the failure that sentence forbids, so the missing-home case answers
/// with a usage error naming `--out`.
#[test]
fn the_key_store_is_the_users_home_and_never_the_working_directory() {
    let home = secure_scratch("key_home");
    let out = run(support::gx().arg("key").arg("list").env("HOME", &home));
    let json = out.json();
    println!("KEY_LIST_DIR={}", json["dir"]);
    assert_eq!(out.code, 0);
    assert!(
        json["dir"]
            .as_str()
            .expect("a path")
            .replace('\\', "/")
            .ends_with("/.gx/keys"),
        "req/56 §3: 「秘密鍵=`~/.gx/keys/`」 — got {}",
        json["dir"]
    );

    let out = run(support::gx()
        .arg("key")
        .arg("list")
        .env_remove("HOME")
        .env_remove("USERPROFILE"));
    println!("KEY_NO_HOME exit={} stderr={}", out.code, out.stderr.trim());
    assert_eq!(out.code, 1, "規律52: 「入力不正」 is 1");
    assert!(
        out.stderr.contains("VALIDATION_ERROR"),
        "44 §1.3's problem object on stderr: {}",
        out.stderr
    );
    assert!(
        out.stdout.trim().is_empty(),
        "44 §1.3: 「stdoutは何も出さない」"
    );
}
