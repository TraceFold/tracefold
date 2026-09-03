// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use crate::atom::{first_line, short_id, KIND_GAP, LAYER_L0, LAYER_L1, LAYER_L2};
use crate::gate::{GateLine, REASON_COMMUTE_BREAK, REASON_EMPTY_CORPUS, REASON_LOD_NOT_MONOTONE};
use crate::manifest::{Caps, DbManifest, EXECUTORS, LAYER_KEYS, ROLES};
use rusqlite::types::Value as SqlValue;
use std::collections::BTreeMap;

pub const REASON_EMPTY: &str = "EMPTY";
pub const REASON_UNTESTABLE: &str = "UNTESTABLE";
pub const REASON_UNKNOWN_VALUE: &str = "UNKNOWN_FILTER_VALUE";
pub const REASON_UNKNOWN_LOD: &str = "UNKNOWN_LOD";
pub const REASON_OVER_CAP: &str = "OVER_CAP";
pub const REASON_OVER_BUDGET: &str = "OVER_BUDGET";
pub const REASON_BAD_CURSOR: &str = "BAD_CURSOR";
pub const REASON_AMBIGUOUS: &str = "AMBIGUOUS_ADDRESS";
pub const REASON_STALE_INDEX: &str = "STALE_INDEX";
pub const REASON_ANSWERED: &str = "ANSWERED";

pub struct Page {
    pub command: &'static str,
    pub rows: Vec<IndexAtom>,
    pub scores: Vec<f64>,
    pub total: Option<usize>,
    pub matched: usize,
    pub unreadable: usize,
    pub gaps_excluded: usize,
    pub cap: usize,
    pub budget_tokens: usize,
    pub lod: usize,
    pub band: Option<String>,
    pub layer: Option<String>,
    pub cursor: Option<String>,
}

impl Page {
    pub fn empty(command: &'static str) -> Self {
        Page {
            command,
            rows: Vec::new(),
            scores: Vec::new(),
            total: None,
            matched: 0,
            unreadable: 0,
            gaps_excluded: 0,
            cap: 0,
            budget_tokens: 0,
            lod: 0,
            band: None,
            layer: None,
            cursor: None,
        }
    }
}

pub struct Outcome {
    pub exit: i32,
    pub reason: &'static str,
    pub text: String,
    pub page: Page,
}

impl Outcome {
    pub fn refused(reason: &'static str, text: String) -> Self {
        Outcome {
            exit: 2,
            reason,
            text,
            page: Page::empty("unknown"),
        }
    }
    pub fn refused_with(reason: &'static str, text: String, page: Page) -> Self {
        Outcome {
            exit: 2,
            reason,
            text,
            page,
        }
    }
    pub fn answered(text: String, page: Page) -> Self {
        Outcome {
            exit: 0,
            reason: REASON_ANSWERED,
            text,
            page,
        }
    }
    pub fn verdict(&self) -> &'static str {
        match self.reason {
            REASON_ANSWERED => "TRUE",
            REASON_EMPTY => "FALSE",
            _ => "UNKNOWN",
        }
    }
}

fn provenance_json(atom: &IndexAtom) -> serde_json::Value {
    serde_json::json!({
        "path": format!("bands/{}/{}", atom.band, atom.path),
        "anchor": atom.anchor,
        "start_line": atom.line_start,
        "end_line": atom.line_end,
        "byte_start": atom.byte_start,
        "byte_end": atom.byte_end,
    })
}

pub fn row_json(atom: &IndexAtom, lod: usize, score: Option<f64>) -> serde_json::Value {
    let mut row = serde_json::json!({
        "id": atom.id,
        "band": atom.band,
        "document": format!("{}/{}", atom.band, atom.path),
        "layer": atom.layer,
        "kind": atom.kind,
        "role": atom.role,
        "executor": atom.executor,
        "evidence": atom.evidence,
        "line": first_line(&atom.content),
    });
    if let Some(found) = score {
        row["score"] = serde_json::json!(found);
    }
    if lod >= 1 {
        row["content"] = serde_json::json!(atom.content);
    }
    if lod >= 2 {
        row["provenance"] = provenance_json(atom);
        let mut relations: Vec<serde_json::Value> = Vec::new();
        if let Some(parent) = &atom.parent {
            relations.push(serde_json::json!({ "type": "parent", "dst": parent }));
        }
        row["relations"] = serde_json::json!(relations);
    }
    row
}

fn envelope(
    verdict: &str,
    reason: &str,
    exit: i32,
    query: serde_json::Value,
    cap: serde_json::Value,
    denominator: serde_json::Value,
    rows: Vec<serde_json::Value>,
    note: String,
) -> String {
    let body = serde_json::json!({
        "schema": 1,
        "verdict": verdict,
        "reason": reason,
        "exit": exit,
        "query": query,
        "cap": cap,
        "denominator": denominator,
        "rows": rows,
        "note": note,
    });
    match serde_json::to_string_pretty(&body) {
        Ok(text) => text,
        Err(error) => format!(
            "{{\"schema\":1,\"verdict\":\"UNKNOWN\",\"reason\":\"WIRE_NOT_SERIALISED\",\"exit\":2,\"note\":{:?}}}",
            error.to_string()
        ),
    }
}

pub fn wire(outcome: &Outcome) -> String {
    let page = &outcome.page;
    let rows: Vec<serde_json::Value> = page
        .rows
        .iter()
        .enumerate()
        .map(|(index, atom)| {
            let score = match page.scores.get(index) {
                Some(found) => Some(*found),
                None => None,
            };
            row_json(atom, page.lod, score)
        })
        .collect();
    envelope(
        outcome.verdict(),
        outcome.reason,
        outcome.exit,
        serde_json::json!({
            "cmd": page.command,
            "band": page.band,
            "layer": page.layer,
            "lod": page.lod,
            "cursor": page.cursor,
        }),
        serde_json::json!({
            "rows": page.cap,
            "budget_tokens": page.budget_tokens,
            "bytes_returned": outcome.text.len(),
        }),
        serde_json::json!({
            "total": page.total,
            "matched": page.matched,
            "returned": page.rows.len(),
            "withheld": page.matched.saturating_sub(page.rows.len()),
            "unscanned": page.unreadable,
            "gaps_excluded": page.gaps_excluded,
        }),
        rows,
        outcome.text.trim_end().to_string(),
    )
}

fn gate_row_json(line: &GateLine) -> serde_json::Value {
    let breakdown: Vec<serde_json::Value> = line
        .breakdown
        .iter()
        .map(|(attribute, document, count)| {
            serde_json::json!({
                "attribute": attribute,
                "document": document,
                "count": count,
            })
        })
        .collect();
    serde_json::json!({
        "name": line.name,
        "verdict": line.verdict,
        "reason": line.reason,
        "count": line.count,
        "denominator": line.denominator,
        "detail": line.detail,
        "breakdown": breakdown,
    })
}

pub fn gate_wire(lines: &[GateLine], exit: i32, note: String) -> String {
    let verdict = match exit {
        0 => "TRUE",
        1 => "FALSE",
        _ => "UNKNOWN",
    };
    let mut reason = crate::gate::REASON_OK;
    for wanted in [crate::atom::VERDICT_UNKNOWN, crate::atom::VERDICT_FAIL] {
        if let Some(found) = lines.iter().find(|line| line.verdict == wanted) {
            reason = found.reason;
            break;
        }
    }
    let rows: Vec<serde_json::Value> = lines.iter().map(gate_row_json).collect();
    envelope(
        verdict,
        reason,
        exit,
        serde_json::json!({
            "cmd": "gate",
            "band": serde_json::Value::Null,
            "layer": serde_json::Value::Null,
            "lod": serde_json::Value::Null,
            "cursor": serde_json::Value::Null,
        }),
        serde_json::Value::Null,
        serde_json::json!({
            "matched": lines.len(),
            "returned": lines.len(),
            "withheld": 0,
            "unscanned": 0,
        }),
        rows,
        note.trim_end().to_string(),
    )
}

pub struct BandLine {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub documents: usize,
    pub atoms: usize,
    pub gaps: usize,
}

pub fn band_lines(connection: &rusqlite::Connection) -> Result<Vec<BandLine>, String> {
    let sql = "SELECT b.id, b.title, b.abstract, \
        count(DISTINCT d.id), \
        sum(CASE WHEN a.kind IS NULL THEN 0 WHEN a.kind = ?1 THEN 0 ELSE 1 END), \
        sum(CASE WHEN a.kind = ?1 THEN 1 ELSE 0 END) \
        FROM bands b \
        LEFT JOIN documents d ON d.band_id = b.id \
        LEFT JOIN atoms a ON a.document_id = d.id \
        GROUP BY b.id, b.title, b.abstract, b.ord ORDER BY b.ord";
    let mut statement = match connection.prepare(sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map(rusqlite::params![KIND_GAP], |row| {
        Ok(BandLine {
            id: row.get(0)?,
            title: row.get(1)?,
            summary: row.get(2)?,
            documents: row.get::<usize, i64>(3)? as usize,
            atoms: row.get::<usize, i64>(4)? as usize,
            gaps: row.get::<usize, i64>(5)? as usize,
        })
    });
    let mut out: Vec<BandLine> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(line) => out.push(line),
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(out)
}

pub struct BandsAnswer {
    pub exit: i32,
    pub reason: &'static str,
    pub text: String,
    pub lines: Vec<BandLine>,
}

pub fn bands(connection: &rusqlite::Connection, manifest: &DbManifest) -> BandsAnswer {
    let lines = match band_lines(connection) {
        Ok(lines) => lines,
        Err(error) => {
            return BandsAnswer {
                exit: 2,
                reason: REASON_UNTESTABLE,
                text: format!("UNTESTABLE: the band table could not be read: {}\n", error),
                lines: Vec::new(),
            }
        }
    };
    if lines.is_empty() {
        return BandsAnswer {
            exit: 2,
            reason: REASON_UNTESTABLE,
            text: format!(
                "UNTESTABLE: the index holds 0 band(s) while db.toml names {}; a listing over nothing is not an answer. Run db compile\n",
                manifest.band_order.len()
            ),
            lines: Vec::new(),
        };
    }
    let width = lines.iter().map(|line| line.id.len()).fold(4usize, std::cmp::max);
    let mut text = format!(
        "{:<width$}  {:>9}  {:>7}  {:>6}  title\n",
        "band",
        "documents",
        "atoms",
        "gaps",
        width = width
    );
    for line in &lines {
        text.push_str(&format!(
            "{:<width$}  {:>9}  {:>7}  {:>6}  {}\n",
            line.id,
            line.documents,
            line.atoms,
            line.gaps,
            line.title,
            width = width
        ));
    }
    text.push_str(&format!(
        "{} band(s) in the order db.toml declares; atoms counts the atoms that carry a claim and gaps counts the bytes between them, so neither number hides inside the other\n",
        lines.len()
    ));
    BandsAnswer {
        exit: 0,
        reason: REASON_ANSWERED,
        text,
        lines,
    }
}

pub fn bands_wire(answer: &BandsAnswer) -> String {
    let rows: Vec<serde_json::Value> = answer
        .lines
        .iter()
        .map(|line| {
            serde_json::json!({
                "id": line.id,
                "title": line.title,
                "abstract": line.summary,
                "documents": line.documents,
                "atoms": line.atoms,
                "gaps": line.gaps,
            })
        })
        .collect();
    let verdict = match answer.reason {
        REASON_ANSWERED => "TRUE",
        REASON_EMPTY => "FALSE",
        _ => "UNKNOWN",
    };
    envelope(
        verdict,
        answer.reason,
        answer.exit,
        serde_json::json!({
            "cmd": "bands",
            "band": serde_json::Value::Null,
            "layer": serde_json::Value::Null,
            "lod": serde_json::Value::Null,
            "cursor": serde_json::Value::Null,
        }),
        serde_json::Value::Null,
        serde_json::json!({
            "total": answer.lines.len(),
            "matched": answer.lines.len(),
            "returned": answer.lines.len(),
            "withheld": 0,
            "unscanned": 0,
            "gaps_excluded": 0,
        }),
        rows,
        answer.text.trim_end().to_string(),
    )
}

#[derive(Clone)]
pub struct IndexAtom {
    pub id: String,
    pub band: String,
    pub path: String,
    pub anchor: String,
    pub layer: String,
    pub kind: String,
    pub role: String,
    pub executor: String,
    pub evidence: String,
    pub ordinal: i64,
    pub line_start: i64,
    pub line_end: i64,
    pub byte_start: i64,
    pub byte_end: i64,
    pub parent: Option<String>,
    pub content: String,
}

pub struct Filters {
    pub band: Option<String>,
    pub layer: Option<String>,
    pub role: Option<String>,
    pub executor: Option<String>,
    pub include_gaps: bool,
}

const ATOM_COLUMNS: &str = "a.id, d.band_id, d.path, a.anchor, a.layer, a.kind, d.role, a.executor, a.evidence, a.ordinal, a.line_start, a.line_end, a.byte_start, a.byte_end, a.parent_id";
const ATOM_SOURCE: &str = " FROM atoms a JOIN documents d ON d.id = a.document_id";
const ATOM_TEXT: &str = " JOIN atom_text t ON t.atom_id = a.id";
const FTS_SOURCE: &str = " FROM atoms_fts JOIN atoms a ON a.id = atoms_fts.atom_id JOIN documents d ON d.id = a.document_id";
const ATOM_ORDER: &str = " ORDER BY d.ord, a.line_start, a.ordinal, a.id";
const SORT_BEFORE: &str = "(d.ord, a.line_start, a.ordinal, a.id) <= (?, ?, ?, ?)";
const SORT_AFTER: &str = "(d.ord, a.line_start, a.ordinal, a.id) > (?, ?, ?, ?)";
pub const COLUMN_LAYER: &str = "a.layer";
pub const COLUMN_BAND: &str = "d.band_id";
pub const COLUMN_ROLE: &str = "d.role";

fn carried(row: &rusqlite::Row, content: String) -> rusqlite::Result<IndexAtom> {
    Ok(IndexAtom {
        id: row.get(0)?,
        band: row.get(1)?,
        path: row.get(2)?,
        anchor: row.get(3)?,
        layer: row.get(4)?,
        kind: row.get(5)?,
        role: row.get(6)?,
        executor: row.get(7)?,
        evidence: row.get(8)?,
        ordinal: row.get(9)?,
        line_start: row.get(10)?,
        line_end: row.get(11)?,
        byte_start: row.get(12)?,
        byte_end: row.get(13)?,
        parent: row.get(14)?,
        content,
    })
}

pub fn read_atoms(connection: &rusqlite::Connection) -> Result<Vec<IndexAtom>, String> {
    let sql = format!(
        "SELECT {}, t.content{}{}{}",
        ATOM_COLUMNS, ATOM_SOURCE, ATOM_TEXT, ATOM_ORDER
    );
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map([], |row| carried(row, row.get(15)?));
    let mut out: Vec<IndexAtom> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(atom) => out.push(atom),
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(out)
}

fn conditions(filters: &Filters) -> (Vec<&'static str>, Vec<SqlValue>) {
    let mut parts: Vec<&'static str> = Vec::new();
    let mut binds: Vec<SqlValue> = Vec::new();
    if let Some(band) = &filters.band {
        parts.push("d.band_id = ?");
        binds.push(SqlValue::Text(band.clone()));
    }
    if let Some(layer) = &filters.layer {
        parts.push("a.layer = ?");
        binds.push(SqlValue::Text(layer.clone()));
    }
    if let Some(role) = &filters.role {
        parts.push("d.role = ?");
        binds.push(SqlValue::Text(role.clone()));
    }
    if let Some(executor) = &filters.executor {
        parts.push("a.executor = ?");
        binds.push(SqlValue::Text(executor.clone()));
    }
    if !filters.include_gaps {
        parts.push("a.kind <> ?");
        binds.push(SqlValue::Text(KIND_GAP.to_string()));
    }
    (parts, binds)
}

fn count_gaps(connection: &rusqlite::Connection, filters: &Filters) -> Result<usize, String> {
    if filters.include_gaps {
        return Ok(0);
    }
    let widened = Filters {
        band: filters.band.clone(),
        layer: filters.layer.clone(),
        role: filters.role.clone(),
        executor: filters.executor.clone(),
        include_gaps: true,
    };
    let (mut parts, mut binds) = conditions(&widened);
    parts.push("a.kind = ?");
    binds.push(SqlValue::Text(KIND_GAP.to_string()));
    let sql = format!("SELECT count(*){}{}", ATOM_SOURCE, where_clause(&parts));
    count_with(connection, &sql, &binds)
}

fn where_clause(parts: &[&str]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    format!(" WHERE {}", parts.join(" AND "))
}

fn count_with(
    connection: &rusqlite::Connection,
    sql: &str,
    binds: &[SqlValue],
) -> Result<usize, String> {
    let mut statement = match connection.prepare(sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let found = statement.query_row(rusqlite::params_from_iter(binds.iter()), |row| {
        row.get::<usize, i64>(0)
    });
    match found {
        Ok(value) => Ok(value as usize),
        Err(error) => Err(error.to_string()),
    }
}

pub fn count_atoms(connection: &rusqlite::Connection) -> Result<usize, String> {
    count_with(connection, "SELECT count(*) FROM atoms", &[])
}

pub fn count_matching(
    connection: &rusqlite::Connection,
    filters: &Filters,
) -> Result<usize, String> {
    let (parts, binds) = conditions(filters);
    let sql = format!("SELECT count(*){}{}", ATOM_SOURCE, where_clause(&parts));
    count_with(connection, &sql, &binds)
}

pub fn histogram(
    connection: &rusqlite::Connection,
    filters: &Filters,
    column: &str,
) -> Result<BTreeMap<String, usize>, String> {
    let (parts, binds) = conditions(filters);
    let sql = format!(
        "SELECT {}, count(*){}{} GROUP BY {}",
        column,
        ATOM_SOURCE,
        where_clause(&parts),
        column
    );
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map(rusqlite::params_from_iter(binds.iter()), |row| {
        Ok((row.get::<usize, String>(0)?, row.get::<usize, i64>(1)?))
    });
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok((key, count)) => {
                        out.insert(key, count as usize);
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(out)
}

pub struct SortKey {
    ord: i64,
    line_start: i64,
    ordinal: i64,
    id: String,
}

impl SortKey {
    fn binds(&self) -> Vec<SqlValue> {
        vec![
            SqlValue::Integer(self.ord),
            SqlValue::Integer(self.line_start),
            SqlValue::Integer(self.ordinal),
            SqlValue::Text(self.id.clone()),
        ]
    }
}

fn sort_key(
    connection: &rusqlite::Connection,
    filters: &Filters,
    wanted: &str,
) -> Result<Option<SortKey>, String> {
    let (mut parts, mut binds) = conditions(filters);
    parts.push("a.id = ?");
    binds.push(SqlValue::Text(wanted.to_string()));
    let sql = format!(
        "SELECT d.ord, a.line_start, a.ordinal, a.id{}{}",
        ATOM_SOURCE,
        where_clause(&parts)
    );
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let found = statement.query_row(rusqlite::params_from_iter(binds.iter()), |row| {
        Ok(SortKey {
            ord: row.get(0)?,
            line_start: row.get(1)?,
            ordinal: row.get(2)?,
            id: row.get(3)?,
        })
    });
    match found {
        Ok(key) => Ok(Some(key)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn rows_up_to(
    connection: &rusqlite::Connection,
    filters: &Filters,
    key: &SortKey,
) -> Result<usize, String> {
    let (mut parts, mut binds) = conditions(filters);
    parts.push(SORT_BEFORE);
    binds.extend(key.binds());
    let sql = format!("SELECT count(*){}{}", ATOM_SOURCE, where_clause(&parts));
    count_with(connection, &sql, &binds)
}

fn page_ids(
    connection: &rusqlite::Connection,
    filters: &Filters,
    after: Option<&SortKey>,
    limit: usize,
) -> Result<Vec<String>, String> {
    let (mut parts, mut binds) = conditions(filters);
    if let Some(key) = after {
        parts.push(SORT_AFTER);
        binds.extend(key.binds());
    }
    let sql = format!(
        "SELECT a.id{}{}{} LIMIT ?",
        ATOM_SOURCE,
        where_clause(&parts),
        ATOM_ORDER
    );
    binds.push(SqlValue::Integer(limit as i64));
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map(rusqlite::params_from_iter(binds.iter()), |row| {
        row.get::<usize, String>(0)
    });
    let mut out: Vec<String> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(id) => out.push(id),
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(out)
}

pub fn load_atoms(
    connection: &rusqlite::Connection,
    ids: &[String],
) -> Result<Vec<IndexAtom>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let holes: Vec<&str> = ids.iter().map(|_| "?").collect();
    let sql = format!(
        "SELECT {}, t.content{}{} WHERE a.id IN ({})",
        ATOM_COLUMNS,
        ATOM_SOURCE,
        ATOM_TEXT,
        holes.join(", ")
    );
    let binds: Vec<SqlValue> = ids.iter().map(|id| SqlValue::Text(id.clone())).collect();
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map(rusqlite::params_from_iter(binds.iter()), |row| {
        carried(row, row.get(15)?)
    });
    let mut held: BTreeMap<String, IndexAtom> = BTreeMap::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(atom) => {
                        held.insert(atom.id.clone(), atom);
                    }
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    let mut out: Vec<IndexAtom> = Vec::new();
    for id in ids {
        match held.remove(id) {
            Some(atom) => out.push(atom),
            None => {
                return Err(format!(
                    "atom {} is in the index but carries no row in atom_text, so its body could not be read; a row with no body is refused rather than rendered empty",
                    short_id(id)
                ))
            }
        }
    }
    Ok(out)
}

pub fn known_bands(connection: &rusqlite::Connection) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut statement = match connection.prepare("SELECT id FROM bands ORDER BY ord") {
        Ok(statement) => statement,
        Err(_) => return out,
    };
    let rows = statement.query_map([], |row| row.get::<usize, String>(0));
    if let Ok(found) = rows {
        for row in found.flatten() {
            out.push(row);
        }
    }
    out
}

fn asked(command: &'static str, filters: &Filters, manifest: &DbManifest, lod: usize) -> Page {
    Page {
        command,
        budget_tokens: manifest.caps.budget_tokens,
        lod,
        band: filters.band.clone(),
        layer: filters.layer.clone(),
        ..Page::empty(command)
    }
}

fn reject_value(name: &str, value: &str, allowed: &[&str], page: Page) -> Outcome {
    Outcome::refused_with(
        REASON_UNKNOWN_VALUE,
        format!(
            "--{} \"{}\" is not one of {}; an unknown value would select nothing and print an empty page as if it were an answer. The matched count on this envelope is 0 because a projection over a value that does not exist was never counted, which is why the verdict beside it is UNKNOWN and not FALSE\n",
            name,
            value,
            allowed.join(" ")
        ),
        page,
    )
}

pub fn validate_filters(
    connection: &rusqlite::Connection,
    filters: &Filters,
    manifest: &DbManifest,
    command: &'static str,
    lod: usize,
) -> Option<Outcome> {
    if let Some(band) = &filters.band {
        let bands = known_bands(connection);
        if !bands.contains(band) {
            let names: Vec<&str> = bands.iter().map(|item| item.as_str()).collect();
            return Some(reject_value("band", band, &names, asked(command, filters, manifest, lod)));
        }
    }
    if let Some(layer) = &filters.layer {
        if !LAYER_KEYS.contains(&layer.as_str()) {
            return Some(reject_value("layer", layer, &LAYER_KEYS, asked(command, filters, manifest, lod)));
        }
    }
    if let Some(role) = &filters.role {
        if !ROLES.contains(&role.as_str()) {
            return Some(reject_value("role", role, &ROLES, asked(command, filters, manifest, lod)));
        }
    }
    if let Some(executor) = &filters.executor {
        let mut allowed: Vec<&str> = EXECUTORS.to_vec();
        allowed.push(crate::atom::UNKNOWN);
        if !allowed.contains(&executor.as_str()) {
            return Some(reject_value("executor", executor, &allowed, asked(command, filters, manifest, lod)));
        }
    }
    None
}

pub fn stale_index(command: &'static str, detail: String) -> Outcome {
    Outcome::refused_with(
        REASON_STALE_INDEX,
        format!(
            "STALE_INDEX: {}. The index no longer speaks for the source, so this question was not asked of it: an answer from a stale index reads like an answer about the corpus, and an empty one reads like a corpus that says nothing. Run db compile\n",
            detail
        ),
        Page {
            command,
            ..Page::empty(command)
        },
    )
}

pub fn freshness_unknown(command: &'static str, detail: String) -> Outcome {
    Outcome::refused_with(
        REASON_UNTESTABLE,
        format!(
            "UNTESTABLE: whether this index still speaks for the source could not be decided: {}. That is not the same as an index known to be stale, and it is not an answer either\n",
            detail
        ),
        Page {
            command,
            ..Page::empty(command)
        },
    )
}

pub fn matches(atom: &IndexAtom, filters: &Filters) -> bool {
    if !filters.include_gaps && atom.kind == KIND_GAP {
        return false;
    }
    if let Some(band) = &filters.band {
        if atom.band != *band {
            return false;
        }
    }
    if let Some(layer) = &filters.layer {
        if atom.layer != *layer {
            return false;
        }
    }
    if let Some(role) = &filters.role {
        if atom.role != *role {
            return false;
        }
    }
    if let Some(executor) = &filters.executor {
        if atom.executor != *executor {
            return false;
        }
    }
    true
}

pub fn atom_line(atom: &IndexAtom) -> String {
    if atom.kind == KIND_GAP {
        let held = atom.content.matches('\n').count();
        return format!(
            "[gap] {} line(s) of separator between the atoms that carry a claim; the anchor is derived from the heading above it and names no heading of its own",
            held
        );
    }
    first_line(&atom.content)
}

pub fn render_row(atom: &IndexAtom, lod: usize, line: &str) -> String {
    let headline = format!(
        "{}  {}/{}#{}  {}",
        atom.layer, atom.band, atom.path, atom.anchor, line
    );
    if lod == 0 {
        return headline;
    }
    let mut out = headline;
    let body = atom.content.trim_end_matches('\n');
    if body.trim_end() != first_line(&atom.content) {
        out.push('\n');
        out.push_str(body);
    }
    if lod == 1 {
        return out;
    }
    out.push('\n');
    out.push_str(&format!(
        "  provenance: {}/{} anchor={} line {}..{} byte {}..{} kind={} executor={} evidence={} ordinal={}",
        atom.band,
        atom.path,
        atom.anchor,
        atom.line_start,
        atom.line_end,
        atom.byte_start,
        atom.byte_end,
        atom.kind,
        atom.executor,
        atom.evidence,
        atom.ordinal
    ));
    match &atom.parent {
        Some(parent) => out.push_str(&format!("\n  relations: parent -> {}", short_id(parent))),
        None => out.push_str("\n  relations: parent -> none (this atom sits directly under its document)"),
    }
    out
}

pub fn render_atom(atom: &IndexAtom, lod: usize) -> String {
    render_row(atom, lod, &atom_line(atom))
}

pub fn render_rows(rows: &[IndexAtom], lod: usize) -> Vec<(String, String, String)> {
    rows.iter()
        .map(|atom| {
            (
                atom.layer.clone(),
                atom.kind.clone(),
                render_atom(atom, lod),
            )
        })
        .collect()
}

pub fn cap_for(
    layers: &BTreeMap<String, usize>,
    filters: &Filters,
    caps: &Caps,
) -> (usize, String) {
    if let Some(layer) = &filters.layer {
        if let Some(cap) = caps.for_layer(layer) {
            return (cap, format!("caps.{} (you declared --layer {})", layer, layer));
        }
    }
    let mut lowest = caps.l0;
    let mut which = LAYER_L0.to_string();
    for layer in layers.keys() {
        if let Some(cap) = caps.for_layer(layer) {
            if cap < lowest {
                lowest = cap;
                which = layer.clone();
            }
        }
    }
    (
        lowest,
        format!(
            "caps.{} (no --layer was declared, so the strictest cap among the layers present applies)",
            which
        ),
    )
}

fn gap_note(hidden: usize) -> String {
    if hidden == 0 {
        return String::new();
    }
    format!(
        ", gaps_excluded: {} (the bytes between the atoms that carry claims; they exist so every byte belongs to exactly one atom, and --include-gaps shows them)",
        hidden
    )
}

fn narrowing_hint(
    connection: &rusqlite::Connection,
    filters: &Filters,
    layers: &BTreeMap<String, usize>,
    total: usize,
) -> String {
    let bands = match histogram(connection, filters, COLUMN_BAND) {
        Ok(found) => found,
        Err(error) => {
            return format!("  the projection could not be counted by band: {}\n", error)
        }
    };
    let roles = match histogram(connection, filters, COLUMN_ROLE) {
        Ok(found) => found,
        Err(error) => {
            return format!("  the projection could not be counted by role: {}\n", error)
        }
    };
    let mut out = String::new();
    let mut offered = 0usize;
    for (flag, counted) in [("--layer", layers), ("--band", &bands), ("--role", &roles)] {
        if counted.len() < 2 {
            continue;
        }
        out.push_str(&format!("  narrow by {}: {:?}\n", flag, counted));
        offered += 1;
    }
    if offered == 0 {
        out.push_str(&format!(
            "  no filter narrows this projection: every one of the {} row(s) carries the same band, role and layer, so each of --band, --role and --layer would return the same {} again\n",
            total, total
        ));
    }
    out.push_str("  page with --cursor begin, then with the exact id printed after next: on each page\n");
    out
}

pub fn ls(
    connection: &rusqlite::Connection,
    manifest: &DbManifest,
    filters: &Filters,
    lod: usize,
    cursor: Option<&str>,
) -> Outcome {
    if lod > 2 {
        return Outcome::refused_with(
            REASON_UNKNOWN_LOD,
            format!("--lod {} is not one of 0 1 2\n", lod),
            asked("ls", filters, manifest, lod),
        );
    }
    if let Some(refusal) = validate_filters(connection, filters, manifest, "ls", lod) {
        return refusal;
    }
    let held = match count_atoms(connection) {
        Ok(held) => held,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if held == 0 {
        return Outcome::refused(
            REASON_UNTESTABLE,
            "UNTESTABLE: the index holds 0 atom(s), so this question could not be asked. Run db compile\n".to_string(),
        );
    }
    let hidden = match count_gaps(connection, filters) {
        Ok(hidden) => hidden,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let describe = |cap: usize, page: Vec<IndexAtom>, matched: usize| Page {
        command: "ls",
        rows: page,
        scores: Vec::new(),
        total: Some(held),
        matched,
        unreadable: 0,
        gaps_excluded: hidden,
        cap,
        budget_tokens: manifest.caps.budget_tokens,
        lod,
        band: filters.band.clone(),
        layer: filters.layer.clone(),
        cursor: cursor.map(|value| value.to_string()),
    };
    let total = match count_matching(connection, filters) {
        Ok(total) => total,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if total == 0 {
        return Outcome::refused_with(
            REASON_EMPTY,
            format!(
                "EMPTY: 0 of {} atom(s) match this projection. The corpus is there and the filters are legal, so this is an empty answer, not an unanswerable question; it is still not a pass\n",
                held
            ),
            describe(0, Vec::new(), 0),
        );
    }
    let layers = match histogram(connection, filters, COLUMN_LAYER) {
        Ok(layers) => layers,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let (cap, cap_source) = cap_for(&layers, filters, &manifest.caps);
    let mut start = 0usize;
    let after: Option<SortKey>;
    match cursor {
        Some(key) if key != "begin" => match sort_key(connection, filters, key) {
            Ok(Some(found)) => {
                start = match rows_up_to(connection, filters, &found) {
                    Ok(start) => start,
                    Err(error) => {
                        return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error))
                    }
                };
                after = Some(found);
            }
            Ok(None) => {
                return Outcome::refused_with(
                    REASON_BAD_CURSOR,
                    format!(
                        "cursor \"{}\" is not the id of a row in this projection ({} row(s)); the cursor is the exact id printed after next: on the previous page, never a prefix of it\n",
                        key, total
                    ),
                    describe(cap, Vec::new(), total),
                )
            }
            Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
        },
        Some(_) => after = None,
        None => {
            if total > cap {
                let mut text = format!(
                    "{} row(s) exceed cap {} from {}; nothing is silently truncated\n",
                    total, cap, cap_source
                );
                text.push_str(&narrowing_hint(connection, filters, &layers, total));
                return Outcome::refused_with(
                    REASON_OVER_CAP,
                    text,
                    describe(cap, Vec::new(), total),
                );
            }
            after = None;
        }
    }
    let ids = match page_ids(connection, filters, after.as_ref(), cap) {
        Ok(ids) => ids,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let found = match load_atoms(connection, &ids) {
        Ok(found) => found,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let budget = manifest.caps.budget_tokens * 4;
    let mut body = String::new();
    let mut kept = 0usize;
    let mut first_over = 0usize;
    for atom in &found {
        let rendered = format!("{}\n", render_atom(atom, lod));
        if kept > 0 && body.len() + rendered.len() > budget {
            break;
        }
        if kept == 0 && rendered.len() > budget {
            first_over = rendered.len();
            break;
        }
        body.push_str(&rendered);
        kept += 1;
    }
    if kept == 0 {
        let mut text = format!(
            "the first row of this projection renders to {} byte on its own, over the whole budget of {} byte ({} token) at lod {}; a page cannot be cut below one row, and an atom is never printed in half\n",
            first_over, budget, manifest.caps.budget_tokens, lod
        );
        text.push_str(&narrowing_hint(connection, filters, &layers, total));
        return Outcome::refused_with(
            REASON_OVER_BUDGET,
            text,
            describe(cap, Vec::new(), total),
        );
    }
    let page: Vec<IndexAtom> = found.into_iter().take(kept).collect();
    let cut = if kept < ids.len() {
        format!(
            ", cut to {} row(s) of the {} the cap allows by the budget of {} byte ({} token)",
            kept,
            ids.len(),
            budget,
            manifest.caps.budget_tokens
        )
    } else {
        String::new()
    };
    let mut text = match cursor {
        Some(_) => format!(
            "rows {}..{} of {} (cap {} from {}), lod {}{}{}\n",
            start + 1,
            start + page.len(),
            total,
            cap,
            cap_source,
            lod,
            cut,
            gap_note(hidden)
        ),
        None => format!(
            "rows {} of {} (cap {} from {}), lod {}{}{}\n",
            page.len(),
            total,
            cap,
            cap_source,
            lod,
            cut,
            gap_note(hidden)
        ),
    };
    text.push_str(&body);
    if start + page.len() < total {
        if let Some(last) = page.last() {
            text.push_str(&format!("next: db ls --cursor {}\n", last.id));
        }
    }
    Outcome::answered(text, describe(cap, page, total))
}

const ADDRESS_MATCH: &str = "(a.id = ?1 OR a.anchor = ?1 OR (d.band_id || '/' || d.path || '#' || a.anchor) = ?1 OR (d.path || '#' || a.anchor) = ?1)";

fn address_ids(connection: &rusqlite::Connection, wanted: &str) -> Result<Vec<String>, String> {
    let sql = format!(
        "SELECT a.id{} WHERE {}{}",
        ATOM_SOURCE, ADDRESS_MATCH, ATOM_ORDER
    );
    let binds = vec![SqlValue::Text(wanted.to_string())];
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map(rusqlite::params_from_iter(binds.iter()), |row| {
        row.get::<usize, String>(0)
    });
    let mut out: Vec<String> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(id) => out.push(id),
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(out)
}

pub fn show(connection: &rusqlite::Connection, wanted: &str, lod: usize) -> Outcome {
    if lod > 2 {
        return Outcome::refused_with(
            REASON_UNKNOWN_LOD,
            format!("--lod {} is not one of 0 1 2\n", lod),
            Page {
                command: "show",
                lod,
                ..Page::empty("show")
            },
        );
    }
    let held = match count_atoms(connection) {
        Ok(held) => held,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if held == 0 {
        return Outcome::refused(
            REASON_UNTESTABLE,
            "UNTESTABLE: the index holds 0 atom(s). Run db compile\n".to_string(),
        );
    }
    let ids = match address_ids(connection, wanted) {
        Ok(ids) => ids,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let named = ids.len();
    if named == 0 {
        return Outcome::refused_with(
            REASON_EMPTY,
            format!(
                "EMPTY: no atom has the id or address \"{}\" among {} atom(s). show takes a full id or an exact address, never a prefix\n",
                wanted, held
            ),
            Page {
                command: "show",
                total: Some(held),
                lod,
                ..Page::empty("show")
            },
        );
    }
    let carried: Vec<String> = ids.iter().take(10).cloned().collect();
    let found = match load_atoms(connection, &carried) {
        Ok(found) => found,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if named > 1 {
        let mut text = format!(
            "\"{}\" names {} atoms; an address that selects more than one is ambiguous and is refused rather than answered with the first\n",
            wanted, named
        );
        for atom in found.iter().take(10) {
            text.push_str(&format!(
                "  {}  {}/{}#{}\n",
                short_id(&atom.id),
                atom.band,
                atom.path,
                atom.anchor
            ));
        }
        return Outcome::refused_with(
            REASON_AMBIGUOUS,
            text,
            Page {
                command: "show",
                total: Some(held),
                matched: named,
                lod,
                ..Page::empty("show")
            },
        );
    }
    match found.first() {
        Some(atom) => Outcome::answered(
            format!("{}\n", render_atom(atom, lod)),
            Page {
                command: "show",
                rows: vec![atom.clone()],
                total: Some(held),
                matched: 1,
                cap: 1,
                lod,
                band: Some(atom.band.clone()),
                layer: Some(atom.layer.clone()),
                ..Page::empty("show")
            },
        ),
        None => Outcome::refused(
            REASON_UNTESTABLE,
            format!(
                "the index counted {} atom(s) at \"{}\" and then returned none; the count and the rows disagree\n",
                named, wanted
            ),
        ),
    }
}

pub fn escape_fts(needle: &str) -> String {
    format!("\"{}\"", needle.replace('"', "\"\""))
}

pub fn hit_line(atom: &IndexAtom, needle: &str) -> String {
    if atom.kind == KIND_GAP {
        return atom_line(atom);
    }
    let wanted = needle.trim().to_lowercase();
    if !wanted.is_empty() {
        for (offset, line) in atom.content.split('\n').enumerate() {
            if line.to_lowercase().contains(&wanted) {
                let trimmed = line.trim_end();
                if offset == 0 {
                    return trimmed.to_string();
                }
                return format!("+{} {}", offset, trimmed);
            }
        }
    }
    atom_line(atom)
}

pub fn find(
    connection: &rusqlite::Connection,
    manifest: &DbManifest,
    needle: &str,
    filters: &Filters,
    limit: usize,
) -> Outcome {
    if needle.trim().is_empty() {
        return Outcome::refused(
            REASON_UNKNOWN_VALUE,
            "an empty needle asks nothing; db find needs a string to look for\n".to_string(),
        );
    }
    if let Some(refusal) = validate_filters(connection, filters, manifest, "find", 0) {
        return refusal;
    }
    if limit < 1 {
        return Outcome::refused_with(
            REASON_UNKNOWN_VALUE,
            "--limit below 1 can never return a row\n".to_string(),
            asked("find", filters, manifest, 0),
        );
    }
    let held = match count_atoms(connection) {
        Ok(held) => held,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if held == 0 {
        return Outcome::refused(
            REASON_UNTESTABLE,
            "UNTESTABLE: the index holds 0 atom(s), so the search could not run. Run db compile\n".to_string(),
        );
    }
    let cap = match &filters.layer {
        Some(layer) => match manifest.caps.for_layer(layer) {
            Some(cap) => cap,
            None => manifest.caps.l0,
        },
        None => manifest.caps.l0,
    };
    if limit > cap {
        return Outcome::refused_with(
            REASON_OVER_CAP,
            format!("--limit {} is over the cap {} that applies to this layer\n", limit, cap),
            Page {
                total: Some(held),
                cap,
                ..asked("find", filters, manifest, 0)
            },
        );
    }
    let (mut parts, filter_binds) = conditions(filters);
    parts.insert(0, "atoms_fts MATCH ?");
    let mut binds: Vec<SqlValue> = vec![SqlValue::Text(escape_fts(needle))];
    binds.extend(filter_binds);
    let total = match count_with(
        connection,
        &format!("SELECT count(*){}{}", FTS_SOURCE, where_clause(&parts)),
        &binds,
    ) {
        Ok(total) => total,
        Err(error) => {
            return Outcome::refused(
                REASON_UNTESTABLE,
                format!("the search failed: {}\n", error),
            )
        }
    };
    let scored = format!(
        "SELECT atoms_fts.atom_id, bm25(atoms_fts){}{} ORDER BY bm25(atoms_fts) ASC, atoms_fts.atom_id ASC LIMIT ?",
        FTS_SOURCE,
        where_clause(&parts)
    );
    let mut page_binds = binds.clone();
    page_binds.push(SqlValue::Integer(limit as i64));
    let mut statement = match connection.prepare(&scored) {
        Ok(statement) => statement,
        Err(error) => {
            return Outcome::refused(
                REASON_UNTESTABLE,
                format!("the search could not be prepared: {}\n", error),
            )
        }
    };
    let rows = statement.query_map(rusqlite::params_from_iter(page_binds.iter()), |row| {
        Ok((row.get::<usize, String>(0)?, row.get::<usize, f64>(1)?))
    });
    let mut filtered: Vec<(String, f64)> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(hit) => filtered.push(hit),
                    Err(error) => {
                        return Outcome::refused(
                            REASON_UNTESTABLE,
                            format!("the search failed while reading rows: {}\n", error),
                        )
                    }
                }
            }
        }
        Err(error) => {
            return Outcome::refused(
                REASON_UNTESTABLE,
                format!("the search failed: {}\n", error),
            )
        }
    }
    if total == 0 {
        return Outcome::refused_with(
            REASON_EMPTY,
            format!(
                "EMPTY: \"{}\" matched 0 of {} atom(s) in the full text index. The index is there and the query is legal, so this is an empty answer, not an unanswerable question\n",
                needle, held
            ),
            Page {
                command: "find",
                total: Some(held),
                cap,
                budget_tokens: manifest.caps.budget_tokens,
                band: filters.band.clone(),
                layer: filters.layer.clone(),
                ..Page::empty("find")
            },
        );
    }
    let ids: Vec<String> = filtered.iter().map(|(id, _)| id.clone()).collect();
    let shown = match load_atoms(connection, &ids) {
        Ok(shown) => shown,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let hidden = match count_gaps(connection, filters) {
        Ok(hidden) => hidden,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    let mut text = format!(
        "find \"{}\": {} hit(s), showing {} (bm25, lower is a better match; find returns the address and the line that matched, never the body - pass the address to db show for that){}\n",
        needle,
        total,
        std::cmp::min(total, limit),
        gap_note(hidden)
    );
    let mut scores: Vec<f64> = Vec::new();
    for (index, atom) in shown.iter().enumerate() {
        let score = match filtered.get(index) {
            Some((_, score)) => *score,
            None => {
                return Outcome::refused(
                    REASON_UNTESTABLE,
                    "a row came back without the score it was ranked by\n".to_string(),
                )
            }
        };
        text.push_str(&format!("{:>9.3}  {}\n", score, render_row(atom, 0, &hit_line(atom, needle))));
        scores.push(score);
    }
    if total > limit {
        text.push_str(&format!(
            "{} further hit(s) are not shown; raise --limit (cap {}) to see them\n",
            total - limit,
            cap
        ));
    }
    Outcome::answered(
        text,
        Page {
            command: "find",
            rows: shown,
            scores,
            total: Some(held),
            matched: total,
            gaps_excluded: hidden,
            cap,
            budget_tokens: manifest.caps.budget_tokens,
            band: filters.band.clone(),
            layer: filters.layer.clone(),
            ..Page::empty("find")
        },
    )
}

pub fn commute_gate(connection: &rusqlite::Connection) -> Vec<GateLine> {
    let mut lines: Vec<GateLine> = Vec::new();
    let all = match read_atoms(connection) {
        Ok(all) => all,
        Err(error) => {
            lines.push(GateLine::unknown("G-Q3", REASON_EMPTY_CORPUS, 0, 0, error));
            return lines;
        }
    };
    if all.is_empty() {
        lines.push(GateLine::unknown(
            "G-Q3",
            REASON_EMPTY_CORPUS,
            0,
            0,
            "0 atom(s) to project; a commuting square over nothing is UNTESTABLE, never a pass".to_string(),
        ));
        return lines;
    }
    let mut checks = 0usize;
    let mut breaks: Vec<String> = Vec::new();
    for layer in [LAYER_L0, LAYER_L1, LAYER_L2] {
        for lod in 0..3usize {
            let filters = Filters {
                band: None,
                layer: Some(layer.to_string()),
                role: None,
                executor: None,
                include_gaps: false,
            };
            let filtered: Vec<IndexAtom> =
                all.iter().filter(|atom| matches(atom, &filters)).cloned().collect();
            let render_then: Vec<String> = render_rows(&filtered, lod)
                .into_iter()
                .map(|(_, _, text)| text)
                .collect();
            let filter_then: Vec<String> = render_rows(&all, lod)
                .into_iter()
                .filter(|(found, kind, _)| found == layer && kind != KIND_GAP)
                .map(|(_, _, text)| text)
                .collect();
            checks += 1;
            if render_then != filter_then {
                breaks.push(format!(
                    "layer {} lod {}: render(filter) has {} row(s), filter(render) has {} row(s)",
                    layer,
                    lod,
                    render_then.len(),
                    filter_then.len()
                ));
            }
        }
    }
    if breaks.is_empty() {
        lines.push(GateLine::pass(
            "G-Q3",
            checks,
            format!(
                "layer, kind and LOD commute over {} square(s): filtering by meaning then rendering gives the same rows as rendering then filtering by the same meaning, which is what keeps the cap a function of layer alone",
                checks
            ),
        ));
    } else {
        lines.push(GateLine::fail(
            "G-Q3",
            REASON_COMMUTE_BREAK,
            breaks.len(),
            checks,
            breaks.join(" | "),
        ));
    }

    let mut monotone_breaks: Vec<String> = Vec::new();
    let sample: Vec<&IndexAtom> = all.iter().take(64).collect();
    for atom in &sample {
        let zero = render_atom(atom, 0);
        let one = render_atom(atom, 1);
        let two = render_atom(atom, 2);
        if !one.contains(&zero) || !two.contains(&one) {
            monotone_breaks.push(format!("{}/{}#{}", atom.band, atom.path, atom.anchor));
        }
    }
    if monotone_breaks.is_empty() {
        lines.push(GateLine::pass(
            "G-Q3b",
            sample.len(),
            format!(
                "LOD is monotone over {} sampled atom(s): everything a shallower level says, a deeper level says too",
                sample.len()
            ),
        ));
    } else {
        lines.push(GateLine::fail(
            "G-Q3b",
            REASON_LOD_NOT_MONOTONE,
            monotone_breaks.len(),
            sample.len(),
            format!("{:?}", monotone_breaks.iter().take(5).collect::<Vec<&String>>()),
        ));
    }
    lines
}
