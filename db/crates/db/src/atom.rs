// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const UNKNOWN: &str = "UNKNOWN";

pub const LAYER_L0: &str = "L0";
pub const LAYER_L1: &str = "L1";
pub const LAYER_L2: &str = "L2";

pub const KIND_HEADING: &str = "heading";
pub const KIND_PARA: &str = "para";
pub const KIND_LIST: &str = "list";
pub const KIND_TABLE_ROW: &str = "table_row";
pub const KIND_CODE: &str = "code";
pub const KIND_DECISION: &str = "decision";
pub const KIND_GAP: &str = "gap";

pub const EVIDENCE_MEASURED: &str = "MEASURED";
pub const EVIDENCE_DERIVED: &str = "DERIVED";
pub const EVIDENCE_OWNER_SAID: &str = "OWNER_SAID";

pub const VERDICT_PASS: &str = "pass";
pub const VERDICT_FAIL: &str = "fail";
pub const VERDICT_UNKNOWN: &str = "unknown";

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Range {
    pub start_line: usize,
    pub end_line: usize,
    pub byte_start: usize,
    pub byte_end: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SourceRef {
    pub document: String,
    pub path: String,
    pub anchor: String,
    pub range: Range,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SemanticAtom {
    pub id: String,
    pub kind: String,
    pub content: String,
    pub layer: String,
    pub parent: Option<String>,
    pub order: u32,
    pub tags: Vec<String>,
    pub source: SourceRef,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StoredAtom {
    pub semantic: SemanticAtom,
    pub executor: String,
    pub evidence: String,
    pub band: String,
}

pub fn atom_id(kind: &str, path: &str, anchor: &str, text: &str) -> String {
    let mut material = String::new();
    for field in [kind, path, anchor, text] {
        material.push_str(field);
        material.push('\u{0}');
    }
    sha256_hex(material.as_bytes())
}

pub fn lineage_key(path: &str, anchor: &str, kind: &str) -> String {
    let mut material = String::new();
    for field in [path, anchor, kind] {
        material.push_str(field);
        material.push('\u{1}');
    }
    sha256_hex(material.as_bytes())
}

pub fn lineage_of(atom: &StoredAtom) -> String {
    lineage_key(
        &atom.semantic.source.path,
        &atom.semantic.source.anchor,
        &atom.semantic.kind,
    )
}

pub fn declared(value: &str) -> bool {
    value != UNKNOWN
}

pub fn undeclared_fields(atom: &StoredAtom) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    if !declared(&atom.semantic.layer) {
        out.push("layer");
    }
    if !declared(&atom.executor) {
        out.push("executor");
    }
    if !declared(&atom.evidence) {
        out.push("evidence");
    }
    out
}

pub fn short_id(id: &str) -> String {
    id.chars().take(12).collect()
}

pub fn first_line(text: &str) -> String {
    let mut lines = text.split('\n');
    match lines.next() {
        Some(line) => line.trim_end().to_string(),
        None => String::new(),
    }
}

pub fn canonical_json<T: Serialize>(value: &T) -> String {
    let raw = serde_json::to_value(value);
    match raw {
        Ok(found) => match serde_json::to_string(&found) {
            Ok(text) => text,
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    }
}
