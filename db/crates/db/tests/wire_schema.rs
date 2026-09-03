// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_db");
static COUNTER: AtomicUsize = AtomicUsize::new(0);

const KEYWORDS: [&str; 14] = [
    "$schema",
    "$id",
    "$ref",
    "$defs",
    "title",
    "description",
    "type",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "enum",
    "const",
    "oneOf",
];

fn schema_path() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut root = crate_dir.clone();
    root.pop();
    root.pop();
    root.join("schema").join("wire.json")
}

fn read_schema() -> Value {
    let path = schema_path();
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("UNTESTABLE: {} could not be read: {}", path.display(), error));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("UNTESTABLE: {} is not json: {}", path.display(), error))
}

fn unsupported_keywords(node: &Value, at: &str, out: &mut Vec<String>) {
    let map = match node.as_object() {
        Some(map) => map,
        None => return,
    };
    for key in map.keys() {
        if !KEYWORDS.contains(&key.as_str()) {
            out.push(format!("{}.{}", at, key));
        }
    }
    for collection in ["properties", "$defs"] {
        if let Some(Value::Object(children)) = map.get(collection) {
            for (name, child) in children {
                unsupported_keywords(child, &format!("{}.{}.{}", at, collection, name), out);
            }
        }
    }
    if let Some(items) = map.get("items") {
        unsupported_keywords(items, &format!("{}.items", at), out);
    }
    if let Some(Value::Array(branches)) = map.get("oneOf") {
        for (index, branch) in branches.iter().enumerate() {
            unsupported_keywords(branch, &format!("{}.oneOf[{}]", at, index), out);
        }
    }
}

fn kind_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn accepts(wanted: &str, found: &str) -> bool {
    wanted == found || (wanted == "number" && found == "integer")
}

fn resolve<'a>(schema: &'a Value, reference: &str) -> &'a Value {
    let name = match reference.strip_prefix("#/$defs/") {
        Some(name) => name,
        None => panic!("UNTESTABLE: {} is not a #/$defs/ reference", reference),
    };
    match schema.get("$defs").and_then(|defs| defs.get(name)) {
        Some(found) => found,
        None => panic!("UNTESTABLE: the schema names {} and does not define it", reference),
    }
}

fn check(schema: &Value, node: &Value, value: &Value, at: &str, bad: &mut Vec<String>) {
    if let Some(Value::String(reference)) = node.get("$ref") {
        check(schema, resolve(schema, reference), value, at, bad);
        return;
    }
    if let Some(Value::Array(branches)) = node.get("oneOf") {
        let mut clean = 0usize;
        for branch in branches {
            let mut errors: Vec<String> = Vec::new();
            check(schema, branch, value, at, &mut errors);
            if errors.is_empty() {
                clean += 1;
            }
        }
        if clean != 1 {
            bad.push(format!(
                "{}: {} of {} shapes accept this body, not exactly one",
                at,
                clean,
                branches.len()
            ));
        }
        return;
    }
    if let Some(wanted) = node.get("const") {
        if wanted != value {
            bad.push(format!("{} is {}, not {}", at, value, wanted));
        }
        return;
    }
    let found = kind_of(value);
    match node.get("type") {
        Some(Value::String(wanted)) => {
            if !accepts(wanted, found) {
                bad.push(format!("{} is {}, not {}", at, found, wanted));
                return;
            }
        }
        Some(Value::Array(wanted)) => {
            let ok = wanted
                .iter()
                .any(|one| matches!(one, Value::String(text) if accepts(text, found)));
            if !ok {
                bad.push(format!("{} is {}, not one of {}", at, found, Value::Array(wanted.clone())));
                return;
            }
        }
        _ => {}
    }
    if let Some(Value::Array(allowed)) = node.get("enum") {
        if !allowed.contains(value) {
            bad.push(format!("{} is {}, not one of {:?}", at, value, allowed));
            return;
        }
    }
    if let (Value::Array(items), Some(rule)) = (value, node.get("items")) {
        for (index, item) in items.iter().enumerate() {
            check(schema, rule, item, &format!("{}[{}]", at, index), bad);
        }
        return;
    }
    let held = match value.as_object() {
        Some(held) => held,
        None => return,
    };
    let empty = serde_json::Map::new();
    let properties = match node.get("properties").and_then(|found| found.as_object()) {
        Some(found) => found,
        None => &empty,
    };
    if let Some(Value::Array(required)) = node.get("required") {
        for key in required {
            if let Value::String(name) = key {
                if !held.contains_key(name) {
                    bad.push(format!("{}.{} is absent", at, name));
                }
            }
        }
    }
    if node.get("additionalProperties") == Some(&Value::Bool(false)) {
        for key in held.keys() {
            if !properties.contains_key(key) {
                bad.push(format!("{}.{} is not a field this schema declares", at, key));
            }
        }
    }
    for (key, rule) in properties {
        if let Some(child) = held.get(key) {
            check(schema, rule, child, &format!("{}.{}", at, key), bad);
        }
    }
}

fn errors_against(schema: &Value, profile: &str, value: &Value) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    let node = serde_json::json!({ "$ref": format!("#/$defs/{}", profile) });
    check(schema, &node, value, "body", &mut bad);
    bad
}

fn errors_against_root(schema: &Value, value: &Value) -> Vec<String> {
    let mut bad: Vec<String> = Vec::new();
    check(schema, schema, value, "body", &mut bad);
    bad
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create target directory");
    for entry in fs::read_dir(from).expect("read source directory") {
        let entry = entry.expect("read directory entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

fn fresh_db(label: &str) -> PathBuf {
    let serial = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("db_wire_{}_{}_{}", std::process::id(), serial, label));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clear the previous copy");
    }
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("n10_4layer");
    copy_tree(&fixture, &dir);
    dir
}

fn run(db: &Path, args: &[&str]) -> String {
    let mut command = Command::new(BIN);
    command.arg("--db").arg(db);
    for argument in args {
        command.arg(argument);
    }
    let output = command.output().expect("the db binary should run");
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn body(db: &Path, args: &[&str]) -> Value {
    let text = run(db, args);
    serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!(
            "UNTESTABLE: db {:?} did not print json on stdout ({}): {}",
            args, error, text
        )
    })
}

fn http_get(port: u16, target: &str, method: &str) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let request = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
        method, target
    );
    stream.write_all(request.as_bytes()).ok()?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw).ok()?;
    let at = raw.find("\r\n\r\n")?;
    Some(raw[at + 4..].to_string())
}

fn collected() -> Vec<(&'static str, &'static str, Value)> {
    let db = fresh_db("bodies");
    let compiled = run(&db, &["compile"]);
    assert!(compiled.contains("source_digest"), "compile should answer: {}", compiled);
    run(&db, &["push"]);

    let mut out: Vec<(&'static str, &'static str, Value)> = vec![
        ("ls answered", "page_wire", body(&db, &["ls", "--band", "arch", "--layer", "L0", "--cursor", "begin", "--json"])),
        ("ls at lod 2", "page_wire", body(&db, &["ls", "--band", "arch", "--lod", "2", "--cursor", "begin", "--json"])),
        ("ls empty", "page_wire", body(&db, &["ls", "--band", "arch", "--layer", "L2", "--json"])),
        ("ls over cap", "page_wire", body(&db, &["ls", "--include-gaps", "--json"])),
        ("ls unknown filter", "page_wire", body(&db, &["ls", "--layer", "L9", "--json"])),
        ("ls bad cursor", "page_wire", body(&db, &["ls", "--band", "arch", "--cursor", "deadbeef", "--json"])),
        ("ls unknown lod", "page_wire", body(&db, &["ls", "--band", "arch", "--lod", "7", "--json"])),
        ("show answered", "page_wire", body(&db, &["show", "Overview", "--lod", "2", "--json"])),
        ("show empty", "page_wire", body(&db, &["show", "no-such-address", "--json"])),
        ("find answered", "page_wire", body(&db, &["find", "regenerable", "--json"])),
        ("find empty", "page_wire", body(&db, &["find", "zzzq-not-in-this-corpus", "--json"])),
        ("find over cap", "page_wire", body(&db, &["find", "regenerable", "--limit", "5000", "--json"])),
        ("bands", "bands_wire", body(&db, &["bands", "--json"])),
        ("gate", "gate_wire", body(&db, &["gate", "--json"])),
    ];

    let moved = fresh_db("stale");
    run(&moved, &["compile"]);
    let document = moved.join("bands").join("arch").join("01_overview.md");
    let held = fs::read_to_string(&document).expect("read the document");
    fs::write(&document, format!("{}\nA line the index has not seen.\n", held)).expect("append");
    out.push(("stale index", "page_wire", body(&moved, &["ls", "--band", "arch", "--json"])));

    let served = fresh_db("served");
    run(&served, &["compile"]);
    let port: u16 = 30000 + (std::process::id() % 9000) as u16;
    let mut child = Command::new(BIN)
        .arg("--db")
        .arg(&served)
        .arg("serve")
        .arg("--port")
        .arg(port.to_string())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("serve should start");
    let mut ready = false;
    for _ in 0..80 {
        if http_get(port, "/v1/bands", "GET").is_some() {
            ready = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if !ready {
        let _ = child.kill();
        panic!("UNTESTABLE: serve never answered on port {}, so the bodies only http produces were never collected", port);
    }
    for (label, profile, target, method) in [
        ("http ls", "page_wire", "/v1/ls?band=arch&layer=L0&cursor=begin", "GET"),
        ("http bands", "bands_wire", "/v1/bands", "GET"),
        ("http gate", "gate_wire", "/v1/gate", "GET"),
        ("http unknown route", "transport_refusal", "/nope", "GET"),
        ("http write method", "transport_refusal", "/v1/ls", "POST"),
        ("http unreadable value", "transport_refusal", "/v1/ls?lod=abc", "GET"),
    ] {
        let raw = match http_get(port, target, method) {
            Some(raw) => raw,
            None => {
                let _ = child.kill();
                panic!("UNTESTABLE: {} {} did not answer", method, target);
            }
        };
        let parsed: Value = match serde_json::from_str(&raw) {
            Ok(parsed) => parsed,
            Err(error) => {
                let _ = child.kill();
                panic!("UNTESTABLE: {} answered {} which is not json: {}", target, error, raw);
            }
        };
        out.push((label, profile, parsed));
    }
    let _ = child.kill();
    let _ = child.wait();
    out
}

#[test]
fn every_body_the_engine_emits_is_the_shape_the_schema_declares() {
    let schema = read_schema();
    let bodies = collected();
    assert!(
        bodies.len() >= 20,
        "the conformance run collected {} body(s); a scan that found nothing is UNTESTABLE, never a pass",
        bodies.len()
    );
    let mut broken: Vec<String> = Vec::new();
    for (label, profile, value) in &bodies {
        let against = errors_against(&schema, profile, value);
        if !against.is_empty() {
            broken.push(format!("{} against {}: {}", label, profile, against.join("; ")));
        }
        let root = errors_against_root(&schema, value);
        if !root.is_empty() {
            broken.push(format!("{} against the schema as a whole: {}", label, root.join("; ")));
        }
    }
    assert!(
        broken.is_empty(),
        "{} of {} body(s) are not the shape schema/wire.json declares, so the schema and the engine disagree about what the wire is:\n{}",
        broken.len(),
        bodies.len(),
        broken.join("\n")
    );
}

#[test]
fn a_field_in_one_side_and_not_the_other_turns_this_red_both_ways() {
    let schema = read_schema();
    let db = fresh_db("controls");
    run(&db, &["compile"]);
    let real = body(&db, &["ls", "--band", "arch", "--layer", "L0", "--cursor", "begin", "--json"]);
    assert!(
        errors_against(&schema, "page_wire", &real).is_empty(),
        "the positive control has to hold before the negative ones mean anything"
    );

    let mut planted = schema.clone();
    planted["$defs"]["page_denominator"]["properties"]["sampled"] =
        serde_json::json!({ "type": "integer" });
    planted["$defs"]["page_denominator"]["required"]
        .as_array_mut()
        .expect("required is a list")
        .push(serde_json::json!("sampled"));
    let schema_ahead = errors_against(&planted, "page_wire", &real);
    assert_eq!(
        schema_ahead,
        vec!["body.denominator.sampled is absent".to_string()],
        "a field the schema declares and the engine does not emit has to be named, not tolerated"
    );

    let mut widened = real.clone();
    widened["denominator"]["sampled"] = serde_json::json!(7);
    let code_ahead = errors_against(&schema, "page_wire", &widened);
    assert_eq!(
        code_ahead,
        vec!["body.denominator.sampled is not a field this schema declares".to_string()],
        "a field the engine emits and the schema does not declare has to be named too, or the schema is a subset and not a contract"
    );

    let mut narrowed = real.clone();
    narrowed
        .as_object_mut()
        .expect("the body is an object")
        .remove("verdict");
    assert!(
        !errors_against(&schema, "page_wire", &narrowed).is_empty(),
        "a body that dropped its verdict must not validate"
    );

    let mut retyped = real.clone();
    retyped["denominator"]["matched"] = serde_json::json!("512");
    assert!(
        !errors_against(&schema, "page_wire", &retyped).is_empty(),
        "a count that arrived as a string must not validate"
    );

    let bands = body(&db, &["bands", "--json"]);
    assert!(
        !errors_against(&schema, "page_wire", &bands).is_empty(),
        "the four profiles are exclusive: a band listing must not pass as a page"
    );
    assert!(
        errors_against_root(&schema, &bands).is_empty(),
        "and it must still be exactly one of them"
    );
}

#[test]
fn a_keyword_neither_side_implements_is_refused_rather_than_skipped() {
    let schema = read_schema();
    let mut found: Vec<String> = Vec::new();
    unsupported_keywords(&schema, "#", &mut found);
    assert!(
        found.is_empty(),
        "schema/wire.json uses {} keyword(s) this test and tools/wire_schema.mjs do not implement, which would be a constraint nobody checks: {:?}",
        found.len(),
        found
    );

    let mut planted = schema.clone();
    planted["$defs"]["cap"]["minProperties"] = serde_json::json!(1);
    let mut caught: Vec<String> = Vec::new();
    unsupported_keywords(&planted, "#", &mut caught);
    assert_eq!(
        caught,
        vec!["#.$defs.cap.minProperties".to_string()],
        "the detector has to see an unimplemented keyword, or its silence on the real schema means nothing"
    );
}
