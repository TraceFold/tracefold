// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use crate::atom::{canonical_json, sha256_hex, Atom};
use crate::extract::DocumentIr;
use crate::manifest::{self, BandManifest, DbManifest};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const JOURNAL_FILE: &str = "semantic.journal.jsonl";
pub const HEAD_FILE: &str = "HEAD";
pub const INDEX_FILE: &str = "semantic.sqlite";
pub const DIGEST_TABLES: [&str; 7] = [
    "bands",
    "documents",
    "atoms",
    "atom_text",
    "relations",
    "journal_index",
    "atoms_fts",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JournalRecord {
    pub seq: u64,
    pub ts: String,
    pub atom_id: String,
    pub lineage: String,
    pub version: u64,
    pub prev_hash: String,
    pub gate_verdict: String,
    pub executor: String,
    pub supersedes: Vec<String>,
}

pub fn journal_dir(db: &Path) -> PathBuf {
    db.join(manifest::JOURNAL_DIR)
}

pub fn journal_path(db: &Path) -> PathBuf {
    journal_dir(db).join(JOURNAL_FILE)
}

pub fn head_path(db: &Path) -> PathBuf {
    journal_dir(db).join(HEAD_FILE)
}

pub fn build_dir(db: &Path) -> PathBuf {
    db.join(manifest::BUILD_DIR)
}

pub fn index_dir(db: &Path) -> PathBuf {
    build_dir(db).join("index")
}

pub fn index_path(db: &Path) -> PathBuf {
    index_dir(db).join(INDEX_FILE)
}

pub fn raw_dir(db: &Path) -> PathBuf {
    build_dir(db).join("raw")
}

pub struct RawStats {
    pub written: usize,
    pub documents: usize,
    pub bytes: usize,
    pub digest: String,
}

pub fn refresh_raw(db: &Path, bands: &[BandManifest]) -> Result<RawStats, String> {
    let root = raw_dir(db);
    if root.exists() {
        if let Err(error) = fs::remove_dir_all(&root) {
            return Err(format!("{} could not be cleared: {}", root.display(), error));
        }
    }
    let mut entries: Vec<String> = Vec::new();
    let mut written = 0usize;
    let mut documents = 0usize;
    let mut bytes = 0usize;
    for band in bands {
        for document in &band.documents {
            let source = band.dir.join(&document.path);
            let body = match fs::read(&source) {
                Ok(body) => body,
                Err(error) => {
                    return Err(format!("{} unreadable: {}", source.display(), error))
                }
            };
            documents += 1;
            bytes += body.len();
            let digest = sha256_hex(&body);
            entries.push(format!("{} {}/{}", digest, band.id, document.path));
            let target = root.join(&digest[..2]).join(&digest);
            if target.is_file() {
                continue;
            }
            write_atomic(&target, &body)?;
            written += 1;
        }
    }
    entries.sort();
    let listing = entries.join("\n");
    write_atomic(&root.join("INDEX"), format!("{}\n", listing).as_bytes())?;
    Ok(RawStats {
        written,
        documents,
        bytes,
        digest: sha256_hex(listing.as_bytes()),
    })
}

pub fn now_stamp() -> String {
    let seconds = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(value) => value.as_secs(),
        Err(_) => 0,
    };
    format!("epoch:{}", seconds)
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    if let Err(error) = fs::create_dir_all(&dir) {
        return Err(format!("create dir {} failed: {}", dir.display(), error));
    }
    let stem = match path.file_name() {
        Some(name) => name.to_string_lossy().to_string(),
        None => "out".to_string(),
    };
    let temp = dir.join(format!(".tmp.{}.{}", stem, std::process::id()));
    {
        let mut handle = match fs::File::create(&temp) {
            Ok(handle) => handle,
            Err(error) => return Err(format!("temp create failed: {}", error)),
        };
        if let Err(error) = handle.write_all(bytes) {
            return Err(format!("temp write failed: {}", error));
        }
        let _ = handle.sync_all();
    }
    match fs::rename(&temp, path) {
        Ok(_) => Ok(()),
        Err(error) => Err(format!("rename failed: {}", error)),
    }
}

pub struct JournalRead {
    pub records: Vec<JournalRecord>,
    pub lines: Vec<String>,
    pub unparsable: Vec<usize>,
}

impl JournalRead {
    pub fn denominator(&self) -> String {
        format!(
            "{} record(s) over {} journal line(s); {} UNKNOWN (line is not a json record, counted, never dropped from the denominator)",
            self.records.len(),
            self.lines.len(),
            self.unparsable.len()
        )
    }
    pub fn max_seq(&self) -> u64 {
        let mut top = 0u64;
        for record in &self.records {
            if record.seq > top {
                top = record.seq;
            }
        }
        top
    }
    pub fn newest_for_lineage(&self, lineage: &str) -> Option<&JournalRecord> {
        let mut best: Option<&JournalRecord> = None;
        for record in &self.records {
            if record.lineage == lineage {
                match best {
                    Some(found) if found.version > record.version => {}
                    _ => best = Some(record),
                }
            }
        }
        best
    }
}

pub fn read_journal(db: &Path) -> JournalRead {
    let path = journal_path(db);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => String::new(),
    };
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            continue;
        }
        lines.push(line.trim_end_matches('\r').to_string());
    }
    let mut records: Vec<JournalRecord> = Vec::new();
    let mut unparsable: Vec<usize> = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        match serde_json::from_str::<JournalRecord>(line) {
            Ok(record) => records.push(record),
            Err(_) => unparsable.push(index + 1),
        }
    }
    JournalRead {
        records,
        lines,
        unparsable,
    }
}

pub fn chain_fold(previous: &str, line: &str) -> String {
    let mut material = String::new();
    material.push_str(previous);
    material.push('\u{0}');
    material.push_str(line);
    sha256_hex(material.as_bytes())
}

pub struct ChainVerdict {
    pub lines: usize,
    pub breaks: Vec<String>,
    pub computed_head: String,
    pub stored_head: Option<String>,
}

pub fn read_head(db: &Path) -> Option<String> {
    match fs::read_to_string(head_path(db)) {
        Ok(text) => Some(text.trim().to_string()),
        Err(_) => None,
    }
}

pub fn verify_chain(db: &Path) -> ChainVerdict {
    let journal = read_journal(db);
    let mut running = String::new();
    let mut breaks: Vec<String> = Vec::new();
    for (index, line) in journal.lines.iter().enumerate() {
        match serde_json::from_str::<JournalRecord>(line) {
            Ok(record) => {
                if record.prev_hash != running {
                    breaks.push(format!(
                        "line {} seq={} carries prev_hash {} but the chain up to it folds to {}",
                        index + 1,
                        record.seq,
                        crate::atom::short_id(&record.prev_hash),
                        crate::atom::short_id(&running)
                    ));
                }
            }
            Err(_) => breaks.push(format!(
                "line {} is not a json record, so its link in the chain cannot be checked",
                index + 1
            )),
        }
        running = chain_fold(&running, line);
    }
    ChainVerdict {
        lines: journal.lines.len(),
        breaks,
        computed_head: running,
        stored_head: read_head(db),
    }
}

pub fn append_journal(db: &Path, records: &mut [JournalRecord]) -> Result<usize, String> {
    if records.is_empty() {
        return Ok(0);
    }
    let existing = read_journal(db);
    let mut running = String::new();
    let mut whole = String::new();
    for line in &existing.lines {
        running = chain_fold(&running, line);
        whole.push_str(line);
        whole.push('\n');
    }
    for record in records.iter_mut() {
        record.prev_hash = running.clone();
        let line = canonical_json(record);
        if line.is_empty() {
            return Err("a journal record did not serialise to json; nothing was appended".to_string());
        }
        running = chain_fold(&running, &line);
        whole.push_str(&line);
        whole.push('\n');
    }
    write_atomic(&journal_path(db), whole.as_bytes())?;
    write_atomic(&head_path(db), format!("{}\n", running).as_bytes())?;
    Ok(records.len())
}

pub fn source_digest(
    db: &Path,
    manifest_doc: &DbManifest,
    bands: &[BandManifest],
) -> Result<String, String> {
    let mut material: Vec<u8> = Vec::new();
    let root = manifest::db_manifest_path(db);
    match fs::read(&root) {
        Ok(bytes) => {
            material.extend_from_slice(b"db.toml\0");
            material.extend_from_slice(&bytes);
            material.push(0);
        }
        Err(error) => return Err(format!("{} unreadable: {}", root.display(), error)),
    }
    for band_id in &manifest_doc.band_order {
        let band = match bands.iter().find(|item| item.id == *band_id) {
            Some(found) => found,
            None => return Err(format!("band \"{}\" did not load; digest refused", band_id)),
        };
        let contract = band.dir.join(manifest::BAND_MANIFEST);
        match fs::read(&contract) {
            Ok(bytes) => {
                material.extend_from_slice(b"band\0");
                material.extend_from_slice(band.id.as_bytes());
                material.push(0);
                material.extend_from_slice(&bytes);
                material.push(0);
            }
            Err(error) => return Err(format!("{} unreadable: {}", contract.display(), error)),
        }
        for document in &band.documents {
            let path = band.dir.join(&document.path);
            match fs::read(&path) {
                Ok(bytes) => {
                    material.extend_from_slice(b"doc\0");
                    material.extend_from_slice(band.id.as_bytes());
                    material.push(b'/');
                    material.extend_from_slice(document.path.as_bytes());
                    material.push(0);
                    material.extend_from_slice(&bytes);
                    material.push(0);
                }
                Err(error) => return Err(format!("{} unreadable: {}", path.display(), error)),
            }
        }
    }
    material.extend_from_slice(b"journal\0");
    if let Ok(bytes) = fs::read(journal_path(db)) {
        material.extend_from_slice(&bytes);
    }
    material.push(0);
    Ok(sha256_hex(&material))
}

pub struct IndexStats {
    pub bands: usize,
    pub documents: usize,
    pub atoms: usize,
    pub fts_rows: usize,
    pub relations: usize,
    pub journal_rows: usize,
}

pub fn open_index(db: &Path) -> Result<rusqlite::Connection, String> {
    let path = index_path(db);
    if !path.is_file() {
        return Err(format!(
            "{} does not exist; run db compile (the index is regenerable and is never the original)",
            path.display()
        ));
    }
    match rusqlite::Connection::open(&path) {
        Ok(connection) => Ok(connection),
        Err(error) => Err(error.to_string()),
    }
}

pub fn rebuild_index(
    db: &Path,
    manifest_doc: &DbManifest,
    bands: &[BandManifest],
    documents: &[DocumentIr],
    atoms: &[Atom],
    journal: &[JournalRecord],
    digest: &str,
) -> Result<IndexStats, String> {
    if let Err(error) = fs::create_dir_all(index_dir(db)) {
        return Err(format!("index dir failed: {}", error));
    }
    let path = index_path(db);
    let _ = fs::remove_file(&path);
    let mut connection = match rusqlite::Connection::open(&path) {
        Ok(connection) => connection,
        Err(error) => return Err(error.to_string()),
    };
    if let Err(error) = connection.busy_timeout(Duration::from_secs(10)) {
        return Err(error.to_string());
    }
    let schema = "
PRAGMA journal_mode=DELETE;
CREATE TABLE bands (id TEXT PRIMARY KEY, title TEXT NOT NULL, abstract TEXT NOT NULL, ord INTEGER NOT NULL);
CREATE TABLE documents (id TEXT PRIMARY KEY, band_id TEXT NOT NULL, path TEXT NOT NULL, ord INTEGER NOT NULL, role TEXT NOT NULL, executor TEXT NOT NULL);
CREATE TABLE atoms (id TEXT PRIMARY KEY, document_id TEXT NOT NULL, parent_id TEXT, layer TEXT NOT NULL, kind TEXT NOT NULL, anchor TEXT NOT NULL, ordinal INTEGER NOT NULL, line_start INTEGER NOT NULL, line_end INTEGER NOT NULL, byte_start INTEGER NOT NULL, byte_end INTEGER NOT NULL, executor TEXT NOT NULL, evidence TEXT NOT NULL);
CREATE TABLE atom_text (atom_id TEXT PRIMARY KEY, content TEXT NOT NULL);
CREATE TABLE relations (src TEXT NOT NULL, dst TEXT NOT NULL, type TEXT NOT NULL);
CREATE TABLE journal_index (seq INTEGER PRIMARY KEY, atom_id TEXT NOT NULL, version INTEGER NOT NULL, ts TEXT NOT NULL);
CREATE VIRTUAL TABLE atoms_fts USING fts5(atom_id UNINDEXED, content, layer UNINDEXED, band UNINDEXED);
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
";
    if let Err(error) = connection.execute_batch(schema) {
        return Err(format!("index schema failed: {}", error));
    }
    let transaction = match connection.transaction() {
        Ok(transaction) => transaction,
        Err(error) => return Err(error.to_string()),
    };
    for band in bands {
        if let Err(error) = transaction.execute(
            "INSERT INTO bands (id, title, abstract, ord) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![band.id, band.title, band.summary, band.ord as i64],
        ) {
            return Err(error.to_string());
        }
    }
    for document in documents {
        let ord = match bands.iter().find(|item| item.id == document.band) {
            Some(band) => match band.documents.iter().position(|item| item.path == document.path) {
                Some(position) => position as i64,
                None => 0,
            },
            None => 0,
        };
        if let Err(error) = transaction.execute(
            "INSERT INTO documents (id, band_id, path, ord, role, executor) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                document.document_id,
                document.band,
                document.path,
                ord,
                document.role,
                document.executor
            ],
        ) {
            return Err(error.to_string());
        }
    }
    let mut relations = 0usize;
    for atom in atoms {
        if let Err(error) = transaction.execute(
            "INSERT INTO atoms (id, document_id, parent_id, layer, kind, anchor, ordinal, line_start, line_end, byte_start, byte_end, executor, evidence) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![
                atom.id,
                atom.provenance.document,
                atom.parent,
                atom.layer,
                atom.kind,
                atom.provenance.anchor,
                atom.order as i64,
                atom.provenance.range.start_line as i64,
                atom.provenance.range.end_line as i64,
                atom.provenance.range.byte_start as i64,
                atom.provenance.range.byte_end as i64,
                atom.executor,
                atom.evidence
            ],
        ) {
            return Err(error.to_string());
        }
        if let Err(error) = transaction.execute(
            "INSERT INTO atom_text (atom_id, content) VALUES (?1, ?2)",
            rusqlite::params![atom.id, atom.content],
        ) {
            return Err(error.to_string());
        }
        if let Err(error) = transaction.execute(
            "INSERT INTO atoms_fts (atom_id, content, layer, band) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![atom.id, atom.content, atom.layer, atom.band],
        ) {
            return Err(error.to_string());
        }
        if let Some(parent) = &atom.parent {
            if let Err(error) = transaction.execute(
                "INSERT INTO relations (src, dst, type) VALUES (?1, ?2, 'parent')",
                rusqlite::params![atom.id, parent],
            ) {
                return Err(error.to_string());
            }
            relations += 1;
        }
    }
    for record in journal {
        if let Err(error) = transaction.execute(
            "INSERT OR REPLACE INTO journal_index (seq, atom_id, version, ts) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![record.seq as i64, record.atom_id, record.version as i64, record.ts],
        ) {
            return Err(error.to_string());
        }
        for target in &record.supersedes {
            if let Err(error) = transaction.execute(
                "INSERT INTO relations (src, dst, type) VALUES (?1, ?2, 'supersedes')",
                rusqlite::params![record.atom_id, target],
            ) {
                return Err(error.to_string());
            }
            relations += 1;
        }
    }
    let layer_text: Vec<String> = manifest_doc
        .layer_names
        .iter()
        .map(|(key, name)| format!("{}={}", key, name))
        .collect();
    let meta_rows: Vec<(&str, String)> = vec![
        ("schema", manifest_doc.schema.to_string()),
        ("source_digest", digest.to_string()),
        ("build_ts", now_stamp()),
        ("atom_count", atoms.len().to_string()),
        ("document_count", documents.len().to_string()),
        ("band_count", bands.len().to_string()),
        ("layers", layer_text.join(" ")),
        (
            "caps",
            format!(
                "L0={} L1={} L2={} budget_tokens={}",
                manifest_doc.caps.l0,
                manifest_doc.caps.l1,
                manifest_doc.caps.l2,
                manifest_doc.caps.budget_tokens
            ),
        ),
    ];
    for (key, value) in meta_rows {
        if let Err(error) = transaction.execute(
            "INSERT INTO meta (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        ) {
            return Err(error.to_string());
        }
    }
    if let Err(error) = transaction.commit() {
        return Err(error.to_string());
    }
    let fts_rows = count_rows(&connection, "atoms_fts")?;
    let journal_rows = count_rows(&connection, "journal_index")?;
    Ok(IndexStats {
        bands: bands.len(),
        documents: documents.len(),
        atoms: atoms.len(),
        fts_rows,
        relations,
        journal_rows,
    })
}

pub fn count_rows(connection: &rusqlite::Connection, table: &str) -> Result<usize, String> {
    let statement = format!("SELECT count(*) FROM {}", table);
    match connection.query_row(&statement, [], |row| row.get::<usize, i64>(0)) {
        Ok(value) => Ok(value as usize),
        Err(error) => Err(error.to_string()),
    }
}

pub fn meta_value(connection: &rusqlite::Connection, key: &str) -> Option<String> {
    match connection.query_row(
        "SELECT value FROM meta WHERE key = ?1",
        rusqlite::params![key],
        |row| row.get::<usize, String>(0),
    ) {
        Ok(value) => Some(value),
        Err(_) => None,
    }
}

pub fn table_dump(connection: &rusqlite::Connection) -> Result<String, String> {
    let mut out = String::new();
    for table in DIGEST_TABLES {
        let mut statement = match connection.prepare(&format!("SELECT * FROM {}", table)) {
            Ok(statement) => statement,
            Err(error) => return Err(format!("{}: {}", table, error)),
        };
        let columns = statement.column_count();
        let rows = statement.query_map([], |row| {
            let mut fields: Vec<String> = Vec::new();
            for index in 0..columns {
                let value: rusqlite::types::Value = row.get(index)?;
                fields.push(match value {
                    rusqlite::types::Value::Null => "NULL".to_string(),
                    rusqlite::types::Value::Integer(number) => number.to_string(),
                    rusqlite::types::Value::Real(number) => number.to_string(),
                    rusqlite::types::Value::Text(text) => text,
                    rusqlite::types::Value::Blob(bytes) => format!("blob:{}", bytes.len()),
                });
            }
            Ok(fields.join("\u{1}"))
        });
        let mut lines: Vec<String> = Vec::new();
        match rows {
            Ok(found) => {
                for row in found {
                    match row {
                        Ok(text) => lines.push(text),
                        Err(error) => return Err(format!("{}: {}", table, error)),
                    }
                }
            }
            Err(error) => return Err(format!("{}: {}", table, error)),
        }
        lines.sort();
        out.push_str(&format!("# table {} rows {}\n", table, lines.len()));
        for line in lines {
            out.push_str(&line);
            out.push('\n');
        }
    }
    let mut meta_lines: Vec<String> = Vec::new();
    let mut statement = match connection.prepare("SELECT key, value FROM meta WHERE key <> 'build_ts'") {
        Ok(statement) => statement,
        Err(error) => return Err(format!("meta: {}", error)),
    };
    let rows = statement.query_map([], |row| {
        Ok(format!(
            "{}\u{1}{}",
            row.get::<usize, String>(0)?,
            row.get::<usize, String>(1)?
        ))
    });
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(text) => meta_lines.push(text),
                    Err(error) => return Err(format!("meta: {}", error)),
                }
            }
        }
        Err(error) => return Err(format!("meta: {}", error)),
    }
    meta_lines.sort();
    out.push_str(&format!(
        "# table meta rows {} (build_ts excluded: it is the clock, not the corpus)\n",
        meta_lines.len()
    ));
    for line in meta_lines {
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

pub fn table_digest(connection: &rusqlite::Connection) -> Result<String, String> {
    let dump = table_dump(connection)?;
    Ok(sha256_hex(dump.as_bytes()))
}
